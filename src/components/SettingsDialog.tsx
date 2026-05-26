import { useState, useEffect } from 'react';
import { getSettings, saveSettings, getThemes } from '../api/settings';
import type { AppSettings } from '../types/game';
import './SettingsDialog.css';

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsDialog({ isOpen, onClose }: SettingsDialogProps) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [themes, setThemes] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      loadData();
    } else {
      // Reset state when closed
      setError(null);
      setSuccess(null);
    }
  }, [isOpen]);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [settingsData, themesData] = await Promise.all([
        getSettings(),
        getThemes()
      ]);
      setSettings(settingsData);
      setThemes(themesData);
    } catch (err: unknown) {
      console.error('Failed to load settings:', err);
      setError('Failed to load settings. Make sure the backend is running.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!settings) return;

    setIsSaving(true);
    setError(null);
    setSuccess(null);

    try {
      await saveSettings(settings);
      setSuccess('Settings saved successfully');
      setTimeout(() => {
        onClose();
      }, 1500);
    } catch (err: unknown) {
      console.error('Failed to save settings:', err);
      setError('Failed to save settings. Please try again.');
    } finally {
      setIsSaving(false);
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    if (!settings) return;

    const { name, value, type } = e.target as HTMLInputElement;
    
    setSettings({
      ...settings,
      [name]: type === 'checkbox' 
        ? (e.target as HTMLInputElement).checked 
        : type === 'number' 
          ? Number(value) 
          : value
    });
    
    // Clear status messages on edit
    setError(null);
    setSuccess(null);
  };

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}>
      <div className="dialog">
        <div className="dialog-header">
          <h2>Settings</h2>
          <button className="dialog-close" onClick={onClose} title="Close">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <div className="dialog-body">
          {isLoading ? (
            <div className="loading-state">Loading settings...</div>
          ) : settings ? (
            <form id="settings-form" className="settings-form" onSubmit={handleSave}>
              <div className="form-group">
                <label htmlFor="api_port">API Port</label>
                <input
                  type="number"
                  id="api_port"
                  name="api_port"
                  className="input-field"
                  value={settings.api_port}
                  onChange={handleChange}
                  min={1024}
                  max={65535}
                  required
                />
              </div>

              <div className="form-group">
                <label htmlFor="nexus_port">Nexus Port</label>
                <input
                  type="number"
                  id="nexus_port"
                  name="nexus_port"
                  className="input-field"
                  value={settings.nexus_port}
                  onChange={handleChange}
                  min={1024}
                  max={65535}
                  required
                />
              </div>

              <div className="form-group">
                <label htmlFor="theme">Theme</label>
                <select
                  id="theme"
                  name="theme"
                  className="input-field"
                  value={settings.theme}
                  onChange={handleChange}
                >
                  {themes.map(t => (
                    <option key={t} value={t}>{t}</option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label htmlFor="auto_refresh_minutes">Auto Refresh (Minutes, 0 to disable)</label>
                <input
                  type="number"
                  id="auto_refresh_minutes"
                  name="auto_refresh_minutes"
                  className="input-field"
                  value={settings.auto_refresh_minutes}
                  onChange={handleChange}
                  min={0}
                />
              </div>

              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="watcher_enabled"
                  name="watcher_enabled"
                  checked={settings.watcher_enabled}
                  onChange={handleChange}
                />
                <label htmlFor="watcher_enabled">Enable Watcher / Process Monitor</label>
              </div>

              <div className="form-group">
                <label htmlFor="watcher_interval_seconds">Watcher Interval (Seconds)</label>
                <input
                  type="number"
                  id="watcher_interval_seconds"
                  name="watcher_interval_seconds"
                  className="input-field"
                  value={settings.watcher_interval_seconds}
                  onChange={handleChange}
                  min={5}
                  disabled={!settings.watcher_enabled}
                />
              </div>

              {error && <div className="error-message">{error}</div>}
              {success && <div className="success-message">{success}</div>}
            </form>
          ) : (
            <div className="error-message">Could not load settings.</div>
          )}
        </div>

        <div className="dialog-footer">
          <button type="button" className="btn btn-secondary" onClick={onClose} disabled={isSaving}>
            Cancel
          </button>
          <button 
            type="submit" 
            form="settings-form" 
            className="btn btn-primary" 
            disabled={isLoading || isSaving || !settings}
          >
            {isSaving ? 'Saving...' : 'Save Settings'}
          </button>
        </div>
      </div>
    </div>
  );
}
