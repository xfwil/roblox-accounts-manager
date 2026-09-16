import React, { useState } from 'react';
import './AddAccountDialog.css';
import { Modal } from './Modal';

interface AddAccountDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onAdd: (cookie: string, group?: string, alias?: string) => Promise<void>;
  groups: string[];
}

export function AddAccountDialog({ isOpen, onClose, onAdd, groups }: AddAccountDialogProps) {
  const [cookie, setCookie] = useState('');
  const [alias, setAlias] = useState('');
  const [group, setGroup] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!cookie.trim()) {
      setError('Cookie is required');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await onAdd(cookie.trim(), group.trim() || undefined, alias.trim() || undefined);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal title="Add an account" description="Connect a Roblox account to your local workspace." onClose={onClose}>

        <form onSubmit={handleSubmit} className="dialog-body">
          {error && <div className="dialog-error">{error}</div>}

          <div className="form-group">
            <label htmlFor="cookie">Roblox Security Cookie (.ROBLOSECURITY)</label>
            <textarea
              id="cookie"
              className="input-field cookie-input"
              value={cookie}
              onChange={(e) => setCookie(e.target.value)}
              placeholder="_|WARNING:-DO-NOT-SHARE-THIS..."
              autoFocus
              required
            />
          </div>

          <div className="form-row">
            <div className="form-group">
              <label htmlFor="alias">Alias (Optional)</label>
              <input
                id="alias"
                type="text"
                className="input-field"
                value={alias}
                onChange={(e) => setAlias(e.target.value)}
                placeholder="Main, Alt 1, etc."
              />
            </div>

            <div className="form-group">
              <label htmlFor="group">Group (Optional)</label>
              <input
                id="group"
                type="text"
                className="input-field"
                value={group}
                onChange={(e) => setGroup(e.target.value)}
                placeholder="General, Bots, etc."
                list="group-options"
              />
              <datalist id="group-options">
                {groups.map(g => (
                  <option key={g} value={g} />
                ))}
              </datalist>
            </div>
          </div>

          <div className="dialog-footer">
            <button type="button" className="btn btn-secondary" onClick={onClose} disabled={loading}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={loading || !cookie.trim()}>
              {loading ? (
                <>
                  <svg className="spin" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="12" y1="2" x2="12" y2="6"></line>
                    <line x1="12" y1="18" x2="12" y2="22"></line>
                    <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
                    <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
                    <line x1="2" y1="12" x2="6" y2="12"></line>
                    <line x1="18" y1="12" x2="22" y2="12"></line>
                    <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"></line>
                    <line x1="16.24" y1="7.76" x2="19.07" y2="4.93"></line>
                  </svg>
                  Validating...
                </>
              ) : (
                'Add Account'
              )}
            </button>
          </div>
        </form>
    </Modal>
  );
}
