import React, { useEffect, useState } from 'react';
import { Network, ArrowRight, Settings } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';

export function KnowledgeGraphOptionalStep() {
  const { completeOnboarding } = useOnboarding();
  const [isCompleting, setIsCompleting] = React.useState(false);
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  const handleSkip = async () => {
    setIsCompleting(true);
    try {
      await completeOnboarding();
      window.location.reload();
    } catch (error) {
      console.error('Failed to complete onboarding:', error);
      setIsCompleting(false);
    }
  };

  const handleOpenSettings = () => {
    // Complete onboarding first so the main app renders
    completeOnboarding().then(() => {
      // Store a flag to open KG settings after reload
      localStorage.setItem('poly_open_kg_settings_after_onboarding', 'true');
      window.location.reload();
    }).catch((error) => {
      console.error('Failed to complete onboarding:', error);
    });
  };

  return (
    <OnboardingContainer
      title="Knowledge Graph (Optional)"
      description="Connect Poly to a local Knowledge Graph for semantic search across meetings."
      step={isMac ? 5 : 4}
      totalSteps={isMac ? 5 : 4}
      hideProgress={true}
    >
      <div className="flex flex-col items-center space-y-8">
        {/* Info Card */}
        <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 p-5 space-y-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-gray-100 flex items-center justify-center">
              <Network className="w-5 h-5 text-gray-600" />
            </div>
            <div>
              <h3 className="font-medium text-gray-900">Local Knowledge Graph</h3>
              <p className="text-sm text-gray-500">Docker + LightRAG + Neo4j</p>
            </div>
          </div>

          <div className="space-y-2 text-sm text-gray-700">
            <p>
              Index meeting transcripts for semantic search and relationship discovery.
            </p>
            <p className="text-gray-500">
              This requires Docker and runs entirely on your machine. You can set it up anytime in Settings.
            </p>
          </div>
        </div>

        {/* CTA Section */}
        <div className="w-full max-w-xs space-y-3">
          <Button
            onClick={handleOpenSettings}
            disabled={isCompleting}
            className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
          >
            <Settings className="w-4 h-4 mr-2" />
            Set Up Knowledge Graph
          </Button>

          <Button
            onClick={handleSkip}
            disabled={isCompleting}
            variant="outline"
            className="w-full h-11"
          >
            Skip for now
            <ArrowRight className="w-4 h-4 ml-2" />
          </Button>

          <p className="text-xs text-center text-gray-500">
            You can always configure this later in Settings.
          </p>
        </div>
      </div>
    </OnboardingContainer>
  );
}
