import React, { useState, useEffect } from 'react';
import type { AccountView } from '../types/account';
import type { PrivacySettings } from '../types/game';
import { 
  changePassword, 
  changeEmail, 
  setDisplayName, 
  setAccountDescription, 
  getPrivacySettings 
} from '../api/utilities';
import './AccountUtilsDialog.css';
import { Modal } from './Modal';

interface AccountUtilsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  account: AccountView | null;
}

type TabType = 'password' | 'email' | 'display_name' | 'description' | 'privacy';

export function AccountUtilsDialog({ isOpen, onClose, account }: AccountUtilsDialogProps) {
  const [activeTab, setActiveTab] = useState<TabType>('password');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Form states
  const [oldPassword, setOldPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  
  const [emailPassword, setEmailPassword] = useState('');
  const [newEmail, setNewEmail] = useState('');
  
  const [newDisplayName, setNewDisplayName] = useState(account?.display_name || '');
  
  const [description, setDescription] = useState(account?.description || '');
  
  const [privacySettings, setPrivacySettings] = useState<PrivacySettings | null>(null);
  const [loadingPrivacy, setLoadingPrivacy] = useState(true); // only read on the Privacy tab, which always starts a fetch

  useEffect(() => {
    if (!isOpen || !account || activeTab !== 'privacy') return;
    let active = true;
    getPrivacySettings(account.id).then(settings => {
      if (active) setPrivacySettings(settings);
    }).catch(err => { if (active) setError(String(err)); })
      .finally(() => { if (active) setLoadingPrivacy(false); });
    return () => { active = false; };
  }, [isOpen, account, activeTab]);

  if (!isOpen || !account) return null;

  const handleAction = async (action: () => Promise<void>, successMsg: string) => {
    setLoading(true);
    setError(null);
    setSuccess(null);
    try {
      await action();
      setSuccess(successMsg);
      // Clear sensitive fields on success
      setOldPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setEmailPassword('');
    } catch (err: unknown) {
      console.error(`Action failed`, err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Operation failed');
    } finally {
      setLoading(false);
    }
  };

  const onChangePassword = (e: React.FormEvent) => {
    e.preventDefault();
    if (newPassword !== confirmPassword) {
      setError("New passwords don't match");
      return;
    }
    handleAction(
      () => changePassword(account.id, oldPassword, newPassword),
      'Password changed successfully'
    );
  };

  const onChangeEmail = (e: React.FormEvent) => {
    e.preventDefault();
    handleAction(
      () => changeEmail(account.id, emailPassword, newEmail),
      'Email change requested successfully'
    );
  };

  const onChangeDisplayName = (e: React.FormEvent) => {
    e.preventDefault();
    handleAction(
      () => setDisplayName(account.id, newDisplayName),
      'Display name updated successfully'
    );
  };

  const onSaveDescription = (e: React.FormEvent) => {
    e.preventDefault();
    handleAction(
      () => setAccountDescription(account.id, description),
      'Description saved successfully'
    );
  };

  return (
    <Modal title="Manage account" description={account.username} onClose={onClose} className="utils-dialog">

        <div className="utils-layout">
          <div className="utils-tabs">
            {(['password', 'email', 'display_name', 'description', 'privacy'] as TabType[]).map(tab => (
              <button
                key={tab}
                className={`utils-tab ${activeTab === tab ? 'active' : ''}`}
                onClick={() => setActiveTab(tab)}
              >
                {tab.split('_').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}
              </button>
            ))}
          </div>

          <div className="utils-content">
            {error && <div className="alert-error utils-alert">{error}</div>}
            {success && <div className="alert-success utils-alert">{success}</div>}

            {activeTab === 'password' && (
              <form onSubmit={onChangePassword} className="utils-form">
                <div className="form-group">
                  <label>Current Password</label>
                  <input
                    type="password"
                    className="input-field"
                    value={oldPassword}
                    onChange={e => setOldPassword(e.target.value)}
                    required
                  />
                </div>
                <div className="form-group">
                  <label>New Password</label>
                  <input
                    type="password"
                    className="input-field"
                    value={newPassword}
                    onChange={e => setNewPassword(e.target.value)}
                    required
                  />
                </div>
                <div className="form-group">
                  <label>Confirm New Password</label>
                  <input
                    type="password"
                    className="input-field"
                    value={confirmPassword}
                    onChange={e => setConfirmPassword(e.target.value)}
                    required
                  />
                </div>
                <div className="utils-form-actions">
                  <button type="submit" className="btn btn-primary" disabled={loading}>
                    {loading ? 'Changing...' : 'Change Password'}
                  </button>
                </div>
              </form>
            )}

            {activeTab === 'email' && (
              <form onSubmit={onChangeEmail} className="utils-form">
                <div className="form-group">
                  <label>Current Password</label>
                  <input
                    type="password"
                    className="input-field"
                    value={emailPassword}
                    onChange={e => setEmailPassword(e.target.value)}
                    required
                  />
                </div>
                <div className="form-group">
                  <label>New Email Address</label>
                  <input
                    type="email"
                    className="input-field"
                    value={newEmail}
                    onChange={e => setNewEmail(e.target.value)}
                    required
                  />
                </div>
                <div className="utils-form-actions">
                  <button type="submit" className="btn btn-primary" disabled={loading}>
                    {loading ? 'Changing...' : 'Change Email'}
                  </button>
                </div>
              </form>
            )}

            {activeTab === 'display_name' && (
              <form onSubmit={onChangeDisplayName} className="utils-form">
                <div className="form-group">
                  <label>New Display Name</label>
                  <input
                    type="text"
                    className="input-field"
                    value={newDisplayName}
                    onChange={e => setNewDisplayName(e.target.value)}
                    required
                    maxLength={20}
                  />
                  <span className="form-hint">Costs 1,000 Robux if changed within the last 7 days.</span>
                </div>
                <div className="utils-form-actions">
                  <button type="submit" className="btn btn-primary" disabled={loading}>
                    {loading ? 'Saving...' : 'Set Display Name'}
                  </button>
                </div>
              </form>
            )}

            {activeTab === 'description' && (
              <form onSubmit={onSaveDescription} className="utils-form">
                <div className="form-group">
                  <label>Account Description</label>
                  <textarea
                    className="input-field utils-textarea"
                    value={description}
                    onChange={e => setDescription(e.target.value)}
                    rows={6}
                    maxLength={1000}
                  />
                </div>
                <div className="utils-form-actions">
                  <button type="submit" className="btn btn-primary" disabled={loading}>
                    {loading ? 'Saving...' : 'Save Description'}
                  </button>
                </div>
              </form>
            )}

            {activeTab === 'privacy' && (
              <div className="utils-form">
                {loadingPrivacy ? (
                  <div className="utils-loading">Loading privacy settings...</div>
                ) : privacySettings ? (
                  <div className="privacy-settings-list">
                    <div className="privacy-item">
                      <span className="privacy-label">Phone Discovery</span>
                      <span className="privacy-value">{privacySettings.phoneDiscovery || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Who can message me</span>
                      <span className="privacy-value">{privacySettings.privateMessagePrivacy || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Who can chat with me in app</span>
                      <span className="privacy-value">{privacySettings.appChatPrivacy || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Who can chat with me in game</span>
                      <span className="privacy-value">{privacySettings.gameChatPrivacy || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Who can see my inventory</span>
                      <span className="privacy-value">{privacySettings.inventoryPrivacy || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Who can trade with me</span>
                      <span className="privacy-value">{privacySettings.tradePrivacy || 'Unknown'}</span>
                    </div>
                    <div className="privacy-item">
                      <span className="privacy-label">Trade Quality Filter</span>
                      <span className="privacy-value">{privacySettings.tradeValue || 'Unknown'}</span>
                    </div>
                  </div>
                ) : (
                  <div className="utils-empty">Failed to load privacy settings.</div>
                )}
              </div>
            )}
          </div>
        </div>
    </Modal>
  );
}
