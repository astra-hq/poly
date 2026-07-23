'use client';

import React from 'react';
import { AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  children: React.ReactNode;
  fallback?: React.ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class GlossaryErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Glossary tab crashed:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div className="p-6 space-y-4">
          <div className="flex items-center gap-2 text-red-600">
            <AlertCircle className="h-5 w-5" />
            <h3 className="font-semibold">Something went wrong</h3>
          </div>
          <p className="text-sm text-gray-600">
            The glossary tab encountered an error. Please report this issue.
          </p>
          {this.state.error && (
            <pre className="text-xs bg-gray-100 p-3 rounded overflow-auto max-h-48">
              {this.state.error.toString()}
              {'\n'}
              {this.state.error.stack}
            </pre>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => this.setState({ hasError: false })}
          >
            Try Again
          </Button>
        </div>
      );
    }

    return this.props.children;
  }
}
