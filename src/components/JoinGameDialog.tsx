import React, { useState } from 'react';
import { joinGame } from '../api/game';
import './JoinGameDialog.css';

interface JoinGameDialogProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string | null;
}

export function JoinGameDialog({ isOpen, onClose, accountId }: JoinGameDialogProps) {
  const [placeId, setPlaceId] = useState('');
  const [jobId, setJobId] = useState('');
  const [followUserId, setFollowUserId] = useState('');
  const [launchArgs, setLaunchArgs] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  if (!isOpen || !accountId) return null;

  const handleJoin = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!placeId) {
      setError('Place ID is required');
      return;
    }

    const parsedPlaceId = parseInt(placeId, 10);
    if (isNaN(parsedPlaceId)) {
      setError('Place ID must be a valid number');
      return;
    }

    setLoading(true);
    setError(null);
    setSuccess(false);

    try {
      await joinGame(
        accountId,
        parsedPlaceId,
        jobId || undefined,
        followUserId ? parseInt(followUserId, 10) : undefined,
        launchArgs || undefined
      );
      setSuccess(true);
      setTimeout(() => {
        onClose();
        setSuccess(false);
      }, 1500);
    } catch (err: unknown) {
      console.error('Failed to join game', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Failed to launch game');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog-content" onClick={e => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Join Game</h2>
          <button className="btn-icon" onClick={onClose}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <form onSubmit={handleJoin} className="dialog-body">
          <div className="form-group">
            <label>Place ID <span className="required">*</span></label>
            <input
              type="text"
              className="input-field"
              placeholder="e.g. 1818"
              value={placeId}
              onChange={e => setPlaceId(e.target.value)}
              disabled={loading}
              autoFocus
            />
          </div>

          <div className="form-group">
            <label>Job ID (Server Instance) <span className="optional">(optional)</span></label>
            <input
              type="text"
              className="input-field"
              placeholder="e.g. e84c4e09-..."
              value={jobId}
              onChange={e => setJobId(e.target.value)}
              disabled={loading}
            />
          </div>

          <div className="form-group">
            <label>Follow User ID <span className="optional">(optional)</span></label>
            <input
              type="text"
              className="input-field"
              placeholder="e.g. 123456"
              value={followUserId}
              onChange={e => setFollowUserId(e.target.value)}
              disabled={loading}
            />
          </div>

          <div className="form-group">
            <label>Extra Launch Args <span className="optional">(optional)</span></label>
            <input
              type="text"
              className="input-field"
              placeholder="--app"
              value={launchArgs}
              onChange={e => setLaunchArgs(e.target.value)}
              disabled={loading}
            />
          </div>

          {error && (
            <div className="alert-error">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="12" y1="8" x2="12" y2="12"></line>
                <line x1="12" y1="16" x2="12.01" y2="16"></line>
              </svg>
              {error}
            </div>
          )}

          {success && (
            <div className="alert-success">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                <polyline points="22 4 12 14.01 9 11.01"></polyline>
              </svg>
              Game launched successfully!
            </div>
          )}

          <div className="dialog-footer">
            <button type="button" className="btn btn-secondary" onClick={onClose} disabled={loading}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={loading || !placeId}>
              {loading ? (
                <>
                  <svg className="spin" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <line x1="12" y1="2" x2="12" y2="6"></line>
                    <line x1="12" y1="18" x2="12" y2="22"></line>
                    <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
                    <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
                    <line x1="2" y1="12" x2="6" y2="12"></line>
                    <line x1="18" y1="12" x2="22" y2="12"></line>
                    <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"></line>
                    <line x1="16.24" y1="4.93" x2="19.07" y2="7.76"></line>
                  </svg>
                  Launching...
                </>
              ) : (
                'Join Game'
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
