use crate::calendar::macos_values::{ns_error_description, read_event};
use crate::calendar::types::{
    CalendarPermissionProbe, CalendarPermissionStatus, CalendarProbeError, CalendarProbeErrorKind,
    NormalizedCalendarEvent, RawCalendarEvent,
};
use block::ConcreteBlock;
use objc::rc::autoreleasepool;
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};
use std::cell::Cell;
use std::ptr;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

const EK_ENTITY_TYPE_EVENT: u64 = 0;
const LOOKBACK_SECONDS: f64 = -30.0 * 24.0 * 60.0 * 60.0;
const LOOKAHEAD_SECONDS: f64 = 365.0 * 24.0 * 60.0 * 60.0;

static EVENTKIT_ACCESS_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static EVENT_STORE: Cell<Option<usize>> = const { Cell::new(None) };
}

// ─────────────────────────────────────────────────────────────────────────────
// Public EventKit bridge (called by apple_provider.rs on macOS)
// ─────────────────────────────────────────────────────────────────────────────

/// Return the current EventKit authorization status without prompting.
pub fn eventkit_authorization_status() -> Result<CalendarPermissionStatus, CalendarProbeError> {
    authorization_status()
}

/// List all visible calendars that support events.
pub fn list_calendars() -> Result<Vec<crate::calendar::types::CalendarInfo>, CalendarProbeError> {
    with_serialized_eventkit_access(|| {
        autoreleasepool(|| {
            let store = event_store()?;

            // SAFETY: [Category 8 — FFI boundary]
            // `calendarsForEntityType:` returns an NSArray of EKCalendar objects.
            let calendars: *mut Object =
                unsafe { msg_send![store, calendarsForEntityType: EK_ENTITY_TYPE_EVENT] };

            if calendars.is_null() {
                return Ok(Vec::new());
            }

            // SAFETY: [Category 8 — FFI boundary]
            // `calendars` is an NSArray; `count` bounds all indexed access.
            let count: usize = unsafe { msg_send![calendars, count] };
            let mut out = Vec::with_capacity(count);

            for i in 0..count {
                // SAFETY: [Category 8 — FFI boundary]
                // `i` is bounded by `count`.
                let calendar: *mut Object = unsafe { msg_send![calendars, objectAtIndex: i] };
                if calendar.is_null() {
                    continue;
                }

                let id = crate::calendar::macos_values::string_property(
                    calendar,
                    sel!(calendarIdentifier),
                )
                .unwrap_or_default();
                let title = crate::calendar::macos_values::string_property(calendar, sel!(title))
                    .unwrap_or_else(|| "Untitled".to_string());

                out.push(crate::calendar::types::CalendarInfo { id, title });
            }

            Ok(out)
        })
    })
}

/// Prompt the user for calendar access and return the resulting status.
pub fn eventkit_request_access() -> Result<CalendarPermissionStatus, CalendarProbeError> {
    with_serialized_eventkit_access(|| {
        autoreleasepool(|| {
            let store = event_store()?;
            request_calendar_access(store)
        })
    })
}

/// Fetch raw calendar events whose time interval overlaps `[start, end]`
/// (both are seconds since the Unix epoch).
pub fn fetch_events(
    start_epoch: f64,
    end_epoch: f64,
) -> Result<Vec<RawCalendarEvent>, CalendarProbeError> {
    with_serialized_eventkit_access(|| {
        autoreleasepool(|| {
            let store = event_store()?;
            let start = ns_date_from_epoch(start_epoch)?;
            let end = ns_date_from_epoch(end_epoch)?;

            // SAFETY: [Category 8 — FFI boundary]
            // `store` is a live EKEventStore; start/end are NSDate instances.
            let predicate: *mut Object = unsafe {
                msg_send![
                    store,
                    predicateForEventsWithStartDate: start
                    endDate: end
                    calendars: ptr::null_mut::<Object>()
                ]
            };

            if predicate.is_null() {
                return Err(CalendarProbeError::new(
                    CalendarProbeErrorKind::RequestFailed,
                    "EventKit failed to create an event query predicate.",
                ));
            }

            // SAFETY: [Category 8 — FFI boundary]
            // `predicate` was returned by EventKit for the same store.
            let events: *mut Object =
                unsafe { msg_send![store, eventsMatchingPredicate: predicate] };
            if events.is_null() {
                return Ok(Vec::new());
            }

            // SAFETY: [Category 8 — FFI boundary]
            // `events` is an NSArray; `count` bounds all indexed access.
            let count: usize = unsafe { msg_send![events, count] };
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                // SAFETY: [Category 8 — FFI boundary]
                // `i` is bounded by `count`.
                let event: *mut Object = unsafe { msg_send![events, objectAtIndex: i] };
                if event.is_null() {
                    continue;
                }
                out.push(read_event(event));
            }
            Ok(out)
        })
    })
}

/// Fetch a single event by its `eventIdentifier` string.
///
/// Returns `None` when the identifier does not match an existing event on any
/// visible calendar.
pub fn fetch_event_by_identifier(id: &str) -> Result<Option<RawCalendarEvent>, CalendarProbeError> {
    with_serialized_eventkit_access(|| {
        autoreleasepool(|| {
            let store = event_store()?;
            let ns_id = ns_string_from_rust(id)?;

            // SAFETY: [Category 8 — FFI boundary]
            // `eventWithIdentifier:` searches all visible calendars on the store.
            let event: *mut Object = unsafe { msg_send![store, eventWithIdentifier: ns_id] };
            if event.is_null() {
                return Ok(None);
            }
            Ok(Some(read_event(event)))
        })
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: NSDate from epoch seconds
// ─────────────────────────────────────────────────────────────────────────────

fn ns_date_from_epoch(epoch: f64) -> Result<*mut Object, CalendarProbeError> {
    let class = Class::get("NSDate").ok_or_else(|| {
        CalendarProbeError::new(
            CalendarProbeErrorKind::EventKitUnavailable,
            "NSDate class is unavailable.",
        )
    })?;
    // SAFETY: [Category 8 — FFI boundary]
    // `dateWithTimeIntervalSince1970:` takes an NSTimeInterval f64.
    let date: *mut Object = unsafe { msg_send![class, dateWithTimeIntervalSince1970: epoch] };
    if date.is_null() {
        return Err(CalendarProbeError::new(
            CalendarProbeErrorKind::RequestFailed,
            "Failed to create NSDate from epoch.",
        ));
    }
    Ok(date)
}

fn ns_string_from_rust(s: &str) -> Result<*mut Object, CalendarProbeError> {
    let class = Class::get("NSString").ok_or_else(|| {
        CalendarProbeError::new(
            CalendarProbeErrorKind::EventKitUnavailable,
            "NSString class is unavailable.",
        )
    })?;
    use std::ffi::CString;
    let cstr = CString::new(s).map_err(|e| {
        CalendarProbeError::new(CalendarProbeErrorKind::RequestFailed, e.to_string())
    })?;
    // SAFETY: [Category 8 — FFI boundary]
    // `stringWithUTF8String:` expects a null-terminated C string.
    let ns: *mut Object = unsafe { msg_send![class, stringWithUTF8String: cstr.as_ptr()] };
    if ns.is_null() {
        return Err(CalendarProbeError::new(
            CalendarProbeErrorKind::RequestFailed,
            "Failed to create NSString from Rust string.",
        ));
    }
    Ok(ns)
}

// ─────────────────────────────────────────────────────────────────────────────
// Existing probe (used by check_calendar_permission command)
// ─────────────────────────────────────────────────────────────────────────────

pub fn probe_eventkit() -> Result<CalendarPermissionProbe, CalendarProbeError> {
    with_serialized_eventkit_access(|| {
        autoreleasepool(|| {
            let store = event_store()?;
            let status = authorization_status()?;
            let readable_status = match status {
                CalendarPermissionStatus::NotDetermined => request_calendar_access(store)?,
                CalendarPermissionStatus::Authorized | CalendarPermissionStatus::FullAccess => {
                    status
                }
                CalendarPermissionStatus::Denied => {
                    return Ok(CalendarPermissionProbe::unavailable(
                        CalendarPermissionStatus::Denied,
                        CalendarProbeError::new(
                            CalendarProbeErrorKind::PermissionDenied,
                            "Calendar access is denied in macOS Privacy settings.",
                        ),
                    ));
                }
                CalendarPermissionStatus::Restricted => {
                    return Ok(CalendarPermissionProbe::unavailable(
                        CalendarPermissionStatus::Restricted,
                        CalendarProbeError::new(
                            CalendarProbeErrorKind::Restricted,
                            "Calendar access is restricted by macOS policy.",
                        ),
                    ));
                }
                CalendarPermissionStatus::WriteOnly => {
                    return Ok(CalendarPermissionProbe::unavailable(
                        CalendarPermissionStatus::WriteOnly,
                        CalendarProbeError::new(
                            CalendarProbeErrorKind::PermissionDenied,
                            "Calendar write-only access cannot read Apple Calendar events.",
                        ),
                    ));
                }
                CalendarPermissionStatus::UnsupportedPlatform
                | CalendarPermissionStatus::Unknown => {
                    return Ok(CalendarPermissionProbe::unavailable(
                        status,
                        CalendarProbeError::new(
                            CalendarProbeErrorKind::UnknownAuthorizationStatus,
                            "EventKit returned an unknown calendar authorization status.",
                        ),
                    ));
                }
            };

            if !readable_status.can_read_events() {
                return Ok(CalendarPermissionProbe::unavailable(
                    readable_status,
                    CalendarProbeError::new(
                        CalendarProbeErrorKind::PermissionDenied,
                        "Calendar read access was not granted.",
                    ),
                ));
            }

            let sample_event = fetch_sample_event(store)?.map(NormalizedCalendarEvent::from);
            Ok(CalendarPermissionProbe::available(
                readable_status,
                sample_event,
            ))
        })
    })
}

fn with_serialized_eventkit_access<T>(
    operation: impl FnOnce() -> Result<T, CalendarProbeError>,
) -> Result<T, CalendarProbeError> {
    let _guard = EVENTKIT_ACCESS_LOCK.lock().map_err(|_| {
        CalendarProbeError::new(
            CalendarProbeErrorKind::RequestFailed,
            "EventKit access lock is unavailable.",
        )
    })?;
    operation()
}

fn event_store() -> Result<*mut Object, CalendarProbeError> {
    if let Some(store) = EVENT_STORE.with(Cell::get) {
        return Ok(store as *mut Object);
    }

    let class = event_store_class()?;
    // SAFETY: [Category 8 — FFI boundary]
    // `class` is the EKEventStore class object. NSObject alloc/init returns a valid object or nil,
    // and nil is checked before the pointer is used by the rest of the module.
    let store: *mut Object = unsafe { msg_send![class, alloc] };
    // SAFETY: [Category 8 — FFI boundary]
    // `store` is the result of `[EKEventStore alloc]`; `init` is the required initializer.
    let store: *mut Object = unsafe { msg_send![store, init] };

    if store.is_null() {
        return Err(CalendarProbeError::new(
            CalendarProbeErrorKind::EventKitUnavailable,
            "Failed to initialize EKEventStore.",
        ));
    }

    EVENT_STORE.with(|cached_store| cached_store.set(Some(store as usize)));
    Ok(store)
}

fn event_store_class() -> Result<&'static Class, CalendarProbeError> {
    Class::get("EKEventStore").ok_or_else(|| {
        CalendarProbeError::new(
            CalendarProbeErrorKind::EventKitUnavailable,
            "EKEventStore class is unavailable; EventKit may not be linked.",
        )
    })
}

fn authorization_status() -> Result<CalendarPermissionStatus, CalendarProbeError> {
    let class = event_store_class()?;
    // SAFETY: [Category 8 — FFI boundary]
    // The receiver is EKEventStore, the entity discriminant is EKEntityTypeEvent, and EventKit
    // returns an NSInteger authorization status that is parsed into a Rust enum immediately.
    let raw: isize =
        unsafe { msg_send![class, authorizationStatusForEntityType: EK_ENTITY_TYPE_EVENT] };
    Ok(CalendarPermissionStatus::from_eventkit_status(raw))
}

fn request_calendar_access(
    store: *mut Object,
) -> Result<CalendarPermissionStatus, CalendarProbeError> {
    let (tx, rx) = mpsc::channel::<Result<bool, String>>();
    let completion = ConcreteBlock::new(move |granted: bool, error: *mut Object| {
        let result = if error.is_null() {
            Ok(granted)
        } else {
            Err(ns_error_description(error).unwrap_or_else(|| {
                "EventKit returned an error while requesting calendar access.".to_string()
            }))
        };
        let _ = tx.send(result);
    })
    .copy();

    // SAFETY: [Category 8 — FFI boundary]
    // `store` is a live EKEventStore and `respondsToSelector:` only queries method availability.
    let supports_full_access: bool = unsafe {
        msg_send![store, respondsToSelector: sel!(requestFullAccessToEventsWithCompletion:)]
    };

    let store_usize = store as usize;
    let completion_usize = &*completion as *const block::Block<(bool, *mut Object), ()> as usize;

    // Prevent the RcBlock from being dropped before the async dispatch runs.
    // EventKit retains the block when it receives it, so it remains valid
    // for the duration of the permission dialog.
    std::mem::forget(completion);

    // EventKit permission dialogs must be triggered from the main thread.
    dispatch::Queue::main().exec_async(move || {
        autoreleasepool(|| {
            let store = store_usize as *mut Object;
            let completion = completion_usize as *mut block::Block<(bool, *mut Object), ()>;
            if supports_full_access {
                // SAFETY: [Category 8 — FFI boundary]
                // The copied block has Objective-C block ABI and remains alive until the bounded receive
                // below completes. The selector is checked at runtime before being sent.
                unsafe {
                    let _: () = msg_send![
                        store,
                        requestFullAccessToEventsWithCompletion: completion
                    ];
                }
            } else {
                // SAFETY: [Category 8 — FFI boundary]
                // Older macOS EventKit uses this selector with EKEntityTypeEvent. The copied block remains
                // alive until the bounded receive below completes.
                unsafe {
                    let _: () = msg_send![
                        store,
                        requestAccessToEntityType: EK_ENTITY_TYPE_EVENT
                        completion: completion
                    ];
                }
            }
        });
    });

    let granted = rx
        .recv_timeout(Duration::from_secs(60))
        .map_err(|e| {
            CalendarProbeError::new(
                CalendarProbeErrorKind::RequestFailed,
                format!("Timed out waiting for EventKit calendar permission response: {e}"),
            )
        })?
        .map_err(|message| {
            CalendarProbeError::new(CalendarProbeErrorKind::RequestFailed, message)
        })?;

    if granted {
        // macOS may not have updated the cached authorization status immediately after the
        // dialog closes. Poll briefly until it reflects the grant (or timeout).
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            let status = authorization_status()?;
            if status.can_read_events() {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        authorization_status()
    } else {
        Ok(CalendarPermissionStatus::Denied)
    }
}

fn fetch_sample_event(store: *mut Object) -> Result<Option<RawCalendarEvent>, CalendarProbeError> {
    let ns_date = Class::get("NSDate").ok_or_else(|| {
        CalendarProbeError::new(
            CalendarProbeErrorKind::EventKitUnavailable,
            "NSDate class is unavailable.",
        )
    })?;

    // SAFETY: [Category 8 — FFI boundary]
    // NSDate accepts `dateWithTimeIntervalSinceNow:` with an NSTimeInterval f64.
    let start: *mut Object =
        unsafe { msg_send![ns_date, dateWithTimeIntervalSinceNow: LOOKBACK_SECONDS] };
    // SAFETY: [Category 8 — FFI boundary]
    // NSDate accepts `dateWithTimeIntervalSinceNow:` with an NSTimeInterval f64.
    let end: *mut Object =
        unsafe { msg_send![ns_date, dateWithTimeIntervalSinceNow: LOOKAHEAD_SECONDS] };

    // SAFETY: [Category 8 — FFI boundary]
    // `store` is a live EKEventStore; start/end are NSDate instances; nil calendars searches all
    // visible calendars in the bounded probe window.
    let predicate: *mut Object = unsafe {
        msg_send![
            store,
            predicateForEventsWithStartDate: start
            endDate: end
            calendars: ptr::null_mut::<Object>()
        ]
    };

    if predicate.is_null() {
        return Err(CalendarProbeError::new(
            CalendarProbeErrorKind::RequestFailed,
            "EventKit failed to create an event query predicate.",
        ));
    }

    // SAFETY: [Category 8 — FFI boundary]
    // `predicate` is returned by EventKit for the same store, and EventKit returns an NSArray.
    let events: *mut Object = unsafe { msg_send![store, eventsMatchingPredicate: predicate] };
    if events.is_null() {
        return Ok(None);
    }

    // SAFETY: [Category 8 — FFI boundary]
    // `events` is an NSArray returned by EventKit; `count` bounds all later indexed access.
    let count: usize = unsafe { msg_send![events, count] };
    if count == 0 {
        return Ok(None);
    }

    // SAFETY: [Category 8 — FFI boundary]
    // `count > 0`, so index 0 is in bounds for the NSArray returned by EventKit.
    let event: *mut Object = unsafe { msg_send![events, objectAtIndex: 0usize] };
    if event.is_null() {
        return Ok(None);
    }

    Ok(Some(read_event(event)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn serialized_eventkit_access_allows_only_one_operation_at_a_time() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                thread::spawn(move || {
                    with_serialized_eventkit_access(|| {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(current, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(5));
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                })
            })
            .collect();

        for handle in handles {
            handle
                .join()
                .expect("worker should not panic")
                .expect("serialized access should succeed");
        }

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }
}
