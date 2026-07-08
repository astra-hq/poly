import React from 'react';
import { motion } from 'framer-motion';
import { Calendar, Play, SkipForward, AlertTriangle, AlertCircle, Square } from 'lucide-react';

export interface SchedulerStatus {
  type:
    | 'candidate_found'
    | 'recording_started'
    | 'recording_skipped_active'
    | 'skipped'
    | 'error'
    | 'stopped';
  event_id?: string;
  title?: string;
  start?: string;
  end?: string;
  reason?: string;
  message?: string;
}

interface CalendarSchedulerStatusProps {
  status: SchedulerStatus | null;
}

const statusConfig: Record<
  string,
  { icon: React.ReactNode; label: string; colorClass: string; bgClass: string }
> = {
  candidate_found: {
    icon: <Calendar className="w-3.5 h-3.5" />,
    label: 'Candidate found',
    colorClass: 'text-blue-700',
    bgClass: 'bg-blue-50',
  },
  recording_started: {
    icon: <Play className="w-3.5 h-3.5" />,
    label: 'Auto-record started',
    colorClass: 'text-green-700',
    bgClass: 'bg-green-50',
  },
  recording_skipped_active: {
    icon: <SkipForward className="w-3.5 h-3.5" />,
    label: 'Skipped — active recording',
    colorClass: 'text-amber-700',
    bgClass: 'bg-amber-50',
  },
  skipped: {
    icon: <SkipForward className="w-3.5 h-3.5" />,
    label: 'Skipped',
    colorClass: 'text-gray-600',
    bgClass: 'bg-gray-50',
  },
  error: {
    icon: <AlertTriangle className="w-3.5 h-3.5" />,
    label: 'Scheduler error',
    colorClass: 'text-red-700',
    bgClass: 'bg-red-50',
  },
  stopped: {
    icon: <Square className="w-3.5 h-3.5" />,
    label: 'Scheduler stopped',
    colorClass: 'text-gray-600',
    bgClass: 'bg-gray-50',
  },
};

export const CalendarSchedulerStatus: React.FC<CalendarSchedulerStatusProps> = ({ status }) => {
  const config = status ? statusConfig[status.type] : null;

  return (
    <motion.div
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -4 }}
      transition={{ duration: 0.2 }}
      className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-medium ${
        config ? `${config.bgClass} ${config.colorClass}` : 'bg-gray-50 text-gray-500'
      }`}
      data-testid="calendar-scheduler-status"
    >
      {config ? (
        <>
          <span className="flex-shrink-0">{config.icon}</span>
          <span className="truncate">
            {config.label}
            {status?.reason ? `: ${status.reason}` : ''}
            {status?.message ? ` — ${status.message}` : ''}
          </span>
        </>
      ) : (
        <>
          <Calendar className="w-3.5 h-3.5 flex-shrink-0" />
          <span>Scheduler idle</span>
        </>
      )}
    </motion.div>
  );
};
