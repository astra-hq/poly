use crate::calendar::types::RawCalendarEvent;
use objc::runtime::{Class, Object, BOOL};
use objc::{msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;

pub(crate) fn read_event(event: *mut Object) -> RawCalendarEvent {
    // SAFETY: [Category 8 — FFI boundary]
    // `isAllDay` and `status` are documented EKEvent properties with stable signatures.
    let is_all_day: bool = unsafe { msg_send![event, isAllDay] };
    let status_code: isize = unsafe { msg_send![event, status] };
    let is_cancelled = status_code == 3;

    let (calendar_id, organizer_name, organizer_email, organizer_is_current_user) = read_event_extras(event);

    RawCalendarEvent {
        identifier: string_property(event, sel!(eventIdentifier))
            .unwrap_or_else(|| "unknown-event".to_string()),
        title: string_property(event, sel!(title)),
        start: date_property(event, sel!(startDate)),
        end: date_property(event, sel!(endDate)),
        attendees: attendees(event),
        notes: string_property(event, sel!(notes)),
        url: url_property(event, sel!(URL)),
        location: string_property(event, sel!(location)),
        is_all_day,
        is_cancelled,
        calendar_id,
        organizer_name,
        organizer_email,
        organizer_is_current_user,
    }
}

fn read_event_extras(event: *mut Object) -> (Option<String>, Option<String>, Option<String>, bool) {
    // SAFETY: [Category 8 — FFI boundary]
    // `calendar` is a documented property on EKEvent; method exists on all supported macOS versions.
    let calendar: *mut Object = unsafe { msg_send![event, calendar] };
    let calendar_id = if calendar.is_null() {
        None
    } else {
        string_property(calendar, sel!(calendarIdentifier))
    };

    // SAFETY: [Category 8 — FFI boundary]
    // `organizer` returns an EKOrganizer?; nil is checked before property access.
    let organizer: *mut Object = unsafe { msg_send![event, organizer] };
    let (organizer_name, organizer_email, organizer_is_current_user) = if organizer.is_null() {
        (None, None, false)
    } else {
        let name = string_property(organizer, sel!(name));
        // EKOrganizer is a subclass of EKParticipant; emailAddress is available as a
        // deprecated convenience on older runtimes and may return nil on macOS 14+.
        let email = string_property(organizer, sel!(emailAddress));
        // EKParticipant `isCurrentUser` returns BOOL.
        let is_current: bool = unsafe { msg_send![organizer, isCurrentUser] };
        (name, email, is_current)
    };

    (calendar_id, organizer_name, organizer_email, organizer_is_current_user)
}

/// Read an NSString property and return it as a Rust String.
pub(crate) fn string_property(object: *mut Object, selector: objc::runtime::Sel) -> Option<String> {
    // SAFETY: [Category 8 — FFI boundary]
    // The caller supplies an NSString-returning property selector documented for this EventKit type.
    let value: *mut Object = unsafe { msg_send![object, performSelector: selector] };
    ns_string(value)
}

pub(crate) fn ns_error_description(error: *mut Object) -> Option<String> {
    // SAFETY: [Category 8 — FFI boundary]
    // NSError `localizedDescription` returns an NSString or nil; `ns_string` validates it.
    let description: *mut Object = unsafe { msg_send![error, localizedDescription] };
    ns_string(description)
}

fn attendees(event: *mut Object) -> Vec<String> {
    // SAFETY: [Category 8 — FFI boundary]
    // `event` is an EKEvent returned by EventKit; `attendees` returns nil or an NSArray.
    let attendees: *mut Object = unsafe { msg_send![event, attendees] };
    if attendees.is_null() {
        return Vec::new();
    }

    // SAFETY: [Category 8 — FFI boundary]
    // `attendees` is an NSArray; `count` bounds all indexed access.
    let count: usize = unsafe { msg_send![attendees, count] };
    let mut normalized = Vec::with_capacity(count);
    for index in 0..count {
        // SAFETY: [Category 8 — FFI boundary]
        // `index` is bounded by `count`, so NSArray access is in bounds.
        let attendee: *mut Object = unsafe { msg_send![attendees, objectAtIndex: index] };
        if attendee.is_null() {
            continue;
        }
        if let Some(email) = string_property(attendee, sel!(emailAddress)) {
            normalized.push(email);
        } else if let Some(name) = string_property(attendee, sel!(name)) {
            normalized.push(name);
        }
    }
    normalized
}

fn url_property(object: *mut Object, selector: objc::runtime::Sel) -> Option<String> {
    // SAFETY: [Category 8 — FFI boundary]
    // The caller supplies an NSURL-returning property selector documented for EKEvent.
    let url: *mut Object = unsafe { msg_send![object, performSelector: selector] };
    if url.is_null() {
        return None;
    }
    // SAFETY: [Category 8 — FFI boundary]
    // NSURL `absoluteString` returns an NSString or nil, which `ns_string` validates.
    let absolute: *mut Object = unsafe { msg_send![url, absoluteString] };
    ns_string(absolute)
}

fn date_property(object: *mut Object, selector: objc::runtime::Sel) -> Option<f64> {
    // SAFETY: [Category 8 — FFI boundary]
    // The caller supplies an NSDate-returning property selector documented for EKEvent.
    let date: *mut Object = unsafe { msg_send![object, performSelector: selector] };
    if date.is_null() {
        return None;
    }
    // SAFETY: [Category 8 — FFI boundary]
    // NSDate `timeIntervalSince1970` returns NSTimeInterval as f64.
    let timestamp: f64 = unsafe { msg_send![date, timeIntervalSince1970] };
    Some(timestamp)
}

fn ns_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }

    // SAFETY: [Category 8 — FFI boundary]
    // EventKit/Foundation returns NSString values whose UTF8String is null-terminated.
    let bytes: *const c_char = unsafe { msg_send![value, UTF8String] };
    if bytes.is_null() {
        return None;
    }

    // SAFETY: [Category 8 — FFI boundary]
    // Foundation guarantees `UTF8String` is null-terminated for NSString; lossy conversion handles
    // any unexpected invalid UTF-8 without panicking.
    Some(
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned(),
    )
}
