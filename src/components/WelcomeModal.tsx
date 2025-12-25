import { useState, useEffect } from "react";
import {
  X,
  FolderSync,
  Share2,
  Users,
  ArrowRight,
  ChevronLeft,
  ChevronRight,
  Sparkles,
  Check,
} from "lucide-react";

interface WelcomeModalProps {
  onClose: () => void;
  onComplete?: () => void;
}

interface FeatureStep {
  id: string;
  icon: React.ReactNode;
  title: string;
  description: string;
  highlights: string[];
}

const FEATURES: FeatureStep[] = [
  {
    id: "create",
    icon: <FolderSync size={36} strokeWidth={1.5} />,
    title: "Create & Sync",
    description:
      "Transform any folder into a secure P2P drive that syncs instantly across all your devices.",
    highlights: [
      "Select any folder to share",
      "Real-time sync across peers",
      "Works on LAN or internet",
    ],
  },
  {
    id: "share",
    icon: <Share2 size={36} strokeWidth={1.5} />,
    title: "Share Securely",
    description:
      "Generate invite links with custom permissions. Control who can view, edit, or manage your files.",
    highlights: [
      "Custom permission levels",
      "Expiring invite links",
      "Revoke access anytime",
    ],
  },
  {
    id: "collaborate",
    icon: <Users size={36} strokeWidth={1.5} />,
    title: "Collaborate Live",
    description:
      "See who's online, track changes in real-time, and resolve conflicts with ease.",
    highlights: [
      "Live presence indicators",
      "File locking support",
      "Smart conflict resolution",
    ],
  },
];

const STORAGE_KEY = "gix_welcome_shown";

export function WelcomeModal({ onClose, onComplete }: WelcomeModalProps) {
  const [currentStep, setCurrentStep] = useState(0);
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const [direction, setDirection] = useState<"next" | "prev">("next");
  const [isAnimating, setIsAnimating] = useState(false);

  const currentFeature = FEATURES[currentStep];
  const isLastStep = currentStep === FEATURES.length - 1;
  const isFirstStep = currentStep === 0;

  const animateToStep = (newStep: number, dir: "next" | "prev") => {
    if (isAnimating || newStep === currentStep) return;
    setIsAnimating(true);
    setDirection(dir);

    // Wait for exit animation, then change step
    setTimeout(() => {
      setCurrentStep(newStep);
      // Wait for enter animation to complete
      setTimeout(() => setIsAnimating(false), 300);
    }, 200);
  };

  const handleNext = () => {
    if (isLastStep) {
      handleFinish();
    } else {
      animateToStep(currentStep + 1, "next");
    }
  };

  const handlePrev = () => {
    if (!isFirstStep) {
      animateToStep(currentStep - 1, "prev");
    }
  };

  const handleFinish = () => {
    if (dontShowAgain) {
      localStorage.setItem(STORAGE_KEY, "true");
    }
    onComplete?.();
    onClose();
  };

  const handleSkip = () => {
    if (dontShowAgain) {
      localStorage.setItem(STORAGE_KEY, "true");
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      handleSkip();
    } else if (e.key === "ArrowRight" || e.key === "Enter") {
      handleNext();
    } else if (e.key === "ArrowLeft") {
      handlePrev();
    }
  };

  const handleStepClick = (index: number) => {
    if (index !== currentStep) {
      animateToStep(index, index > currentStep ? "next" : "prev");
    }
  };

  return (
    <div className="welcome-overlay" onClick={handleSkip}>
      <div
        className="welcome-modal"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-labelledby="welcome-title"
      >
        <button className="modal-close" onClick={handleSkip} aria-label="Close">
          <X size={18} />
        </button>

        {/* Header with branding */}
        <div className="welcome-header">
          <div className="welcome-badge">
            <Sparkles size={14} />
            <span>Welcome to Gix</span>
          </div>
          <h2 id="welcome-title">P2P Drive Sharing</h2>
          <p className="welcome-subtitle">
            Secure, decentralized file sharing without limits
          </p>
        </div>

        {/* Step indicator */}
        <div className="step-indicators">
          {FEATURES.map((feature, index) => (
            <button
              key={feature.id}
              className={`step-dot ${index === currentStep ? "active" : ""} ${
                index < currentStep ? "completed" : ""
              }`}
              onClick={() => handleStepClick(index)}
              aria-label={`Go to ${feature.title}`}
              aria-current={index === currentStep ? "step" : undefined}
            />
          ))}
        </div>

        {/* Feature content with animation */}
        <div
          className={`welcome-content ${isAnimating ? `animating-${direction}` : "visible"}`}
          key={currentFeature.id}
        >
          <div className="feature-icon">{currentFeature.icon}</div>

          <h3 className="feature-title">{currentFeature.title}</h3>

          <p className="feature-description">{currentFeature.description}</p>

          <ul className="feature-highlights">
            {currentFeature.highlights.map((highlight, index) => (
              <li key={index} style={{ animationDelay: `${index * 80}ms` }}>
                <span className="highlight-icon">
                  <Check size={14} strokeWidth={2.5} />
                </span>
                {highlight}
              </li>
            ))}
          </ul>
        </div>

        {/* Footer */}
        <div className="welcome-footer">
          <label className="dont-show-checkbox">
            <input
              type="checkbox"
              checked={dontShowAgain}
              onChange={(e) => setDontShowAgain(e.target.checked)}
            />
            <span>Don't show again</span>
          </label>

          <div className="welcome-actions">
            {!isFirstStep && (
              <button
                className="btn-secondary"
                onClick={handlePrev}
                aria-label="Previous step"
                disabled={isAnimating}
              >
                <ChevronLeft size={16} />
                Back
              </button>
            )}

            <button
              className="btn-primary"
              onClick={handleNext}
              aria-label={isLastStep ? "Get started" : "Next step"}
              disabled={isAnimating}
            >
              {isLastStep ? (
                <>
                  Get Started
                  <ArrowRight size={16} />
                </>
              ) : (
                <>
                  Next
                  <ChevronRight size={16} />
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Hook to manage welcome modal visibility
 */
export function useWelcomeModal(): {
  showWelcome: boolean;
  setShowWelcome: (show: boolean) => void;
  resetWelcome: () => void;
} {
  const [showWelcome, setShowWelcome] = useState(false);

  useEffect(() => {
    const hasShown = localStorage.getItem(STORAGE_KEY);
    if (!hasShown) {
      // Small delay for better UX on first load
      const timer = setTimeout(() => setShowWelcome(true), 500);
      return () => clearTimeout(timer);
    }
  }, []);

  const resetWelcome = () => {
    localStorage.removeItem(STORAGE_KEY);
    setShowWelcome(true);
  };

  return { showWelcome, setShowWelcome, resetWelcome };
}
