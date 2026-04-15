import { memo } from "react";
import { SettingsModal } from "../settings/SettingsModal";
import { HelpModal } from "../help/HelpModal";
import { OnboardingModal } from "../onboarding";
import type { Settings } from "../../types/settings";
import type { Theme } from "../../hooks/useTheme";

interface AppModalsProps {
  settingsOpen: boolean;
  onSettingsClose: () => void;
  onThemeChange: (theme: Theme) => void;
  onSettingsSaved: (settings: Settings) => void;
  onClearData: () => Promise<void>;
  onAutoIndexAllDrives?: () => Promise<void>;

  helpOpen: boolean;
  onHelpClose: () => void;
  onRestartTour?: () => void;

  showOnboarding: boolean;
  onCompleteOnboarding: () => void;
  onSkipOnboarding: () => void;
}

export const AppModals = memo(function AppModals(props: AppModalsProps) {
  return (
    <>
      <SettingsModal
        isOpen={props.settingsOpen}
        onClose={props.onSettingsClose}
        onThemeChange={props.onThemeChange}
        onSettingsSaved={props.onSettingsSaved}
        onClearData={props.onClearData}
        onAutoIndexAllDrives={props.onAutoIndexAllDrives}
      />
      <HelpModal isOpen={props.helpOpen} onClose={props.onHelpClose} onRestartTour={props.onRestartTour} />
      <OnboardingModal
        isOpen={props.showOnboarding}
        onComplete={props.onCompleteOnboarding}
        onSkip={props.onSkipOnboarding}
      />
    </>
  );
});
