import { useState, useEffect } from "react";
import {
  X,
  FolderSync,
  Share2,
  Shield,
  Users,
  Link2,
  ArrowRight,
  ChevronLeft,
  ChevronRight,
  Sparkles,
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
  details: string[];
}

const FEATURES: FeatureStep[] = [
  {
    id: "create",
    icon: <FolderSync size={32} />,
    title: "Create a Shared Drive",
    description: "Turn any folder into a P2P shared drive that syncs in real-time.",
    details: [
      "Select any folder on your computer",
      "Your files are encrypted end-to-end",
      "Changes sync automatically across all peers",
      "Works over LAN or internet",
    ],
  },
  {
    id: "share",
    icon: <Share2 size={32} />,
    title: "Share with Anyone",
    description: "Generate invite links to share your drives securely with others.",
    details: [
      "Create invite links with custom permissions",
      "Set expiration time for links",
      "Control read, write, or admin access",
      "Revoke access anytime",
    ],
  },
  {
    id: "collaborate",
    icon: <Users size={32} />,
    title: "Real-time Collaboration",
    description: "See who is online and what they are working on.",
    details: [
      "View online collaborators in real-time",
      "Lock files to prevent conflicts",
      "Activity feed shows all changes",
      "Resolve sync conflicts easily",
    ],
  },
  {
    id: "security",
    icon: <Shield size={32} />,
    title: "End-to-End Encryption",
    description: "Your files are secured with enterprise-grade cryptography.",
    details: [
      "ChaCha20-Poly1305 encryption",
      "Ed25519 identity keys",
      "X25519 key exchange",
      "No central server stores your data",
    ],
  },
  {
    id: "join",
    icon: <Link2 size={32} />,
    title: "Join Shared Drives",
    description: "Accept invite links to join drives shared by others.",
    details: [
      "Click invite links to join instantly",
      "Files download automatically",
      "Your changes sync back to others",
      "Leave anytime with one click",
    ],
  },
];

const STORAGE_KEY = "gix_welcome_shown";

export function WelcomeModal({ onClose, onComplete }: WelcomeModalProps) {
  const [currentStep, setCurrentStep] = useState(0);
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const currentFeature = FEATURES[currentStep];
  const isLastStep = currentStep === FEATURES.length - 1;
  const isFirstStep = currentStep === 0;

  const handleNext = () => {
    if (isLastStep) {
      handleFinish();
    } else {
      setCurrentStep((prev) => prev + 1);
    }
  };

  const handlePrev = () => {
    if (!isFirstStep) {
      setCurrentStep((prev) => prev - 1);
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
              onClick={() => setCurrentStep(index)}
              aria-label={`Go to ${feature.title}`}
              aria-current={index === currentStep ? "step" : undefined}
            />
          ))}
        </div>

        {/* Feature content */}
        <div className="welcome-content">
          <div className="feature-icon">{currentFeature.icon}</div>
          <h3 className="feature-title">{currentFeature.title}</h3>
          <p className="feature-description">{currentFeature.description}</p>

          <ul className="feature-details">
            {currentFeature.details.map((detail, index) => (
              <li key={index}>
                <span className="detail-bullet" />
                {detail}
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
            <span>Do not show again</span>
          </label>

          <div className="welcome-actions">
            {!isFirstStep && (
              <button
                className="btn-secondary"
                onClick={handlePrev}
                aria-label="Previous step"
              >
                <ChevronLeft size={16} />
                Back
              </button>
            )}

            <button
              className="btn-primary"
              onClick={handleNext}
              aria-label={isLastStep ? "Get started" : "Next step"}
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
