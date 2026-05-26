import React, { useState, useRef, useEffect } from 'react';
import { joinGame, resolveShareLink, resolvePrivateServer } from '../api/game';
import { useRecentHistory } from '../hooks/useRecentHistory';
import { usePresets } from '../hooks/usePresets';
import './JoinGameBar.css';

interface JoinGameBarProps {
  selectedAccountIds: string[];
}

/**
 * Extract Place ID from input. Accepts:
 * - Plain number: 126884695634066
 * - Game URL: https://www.roblox.com/games/126884695634066/...
 */
function parsePlaceId(input: string): number | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  if (/^\d+$/.test(trimmed)) return parseInt(trimmed, 10);

  try {
    const url = new URL(trimmed.startsWith('http') ? trimmed : `https://${trimmed}`);
    const match = url.pathname.match(/\/games\/(\d+)/);
    if (match) return parseInt(match[1], 10);
  } catch { /* not a URL */ }

  const numMatch = trimmed.match(/(\d{4,})/);
  if (numMatch) return parseInt(numMatch[1], 10);

  return null;
}

/**
 * Extract link code from a Private Server link. Accepts:
 * - Old format: https://www.roblox.com/games/123/...?privateServerLinkCode=XXXXX
 * - New format: https://www.roblox.com/share?code=XXXXX&type=Server
 * - Just the code itself
 */
function parseLinkCode(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  try {
    const url = new URL(trimmed.startsWith('http') ? trimmed : `https://${trimmed}`);

    // Old format: ?privateServerLinkCode=XXXXX
    const psCode = url.searchParams.get('privateServerLinkCode');
    if (psCode) return psCode;

    // New format: /share?code=XXXXX
    const shareCode = url.searchParams.get('code');
    if (shareCode) return shareCode;
  } catch { /* not a URL */ }

  // If it looks like a raw code (hex string, no spaces), use as-is
  if (/^[a-f0-9]{16,}$/i.test(trimmed)) return trimmed;

  return null;
}

/** Shorten a URL or value for display in the recent list */
function shortenLabel(value: string): string {
  // If it's just a number, return as-is
  if (/^\d+$/.test(value.trim())) return value.trim();

  // Try to extract meaningful parts from URLs
  try {
    const url = new URL(value.startsWith('http') ? value : `https://${value}`);
    const placeMatch = url.pathname.match(/\/games\/(\d+)(?:\/([^/?]+))?/);
    if (placeMatch) {
      const name = placeMatch[2]?.replace(/-/g, ' ') || '';
      return name ? `${placeMatch[1]} — ${name}` : placeMatch[1];
    }
    if (url.pathname.includes('/share')) {
      const code = url.searchParams.get('code') || '';
      return `Share: ${code.substring(0, 12)}…`;
    }
  } catch { /* not a URL */ }

  // Truncate long strings
  return value.length > 40 ? value.substring(0, 37) + '…' : value;
}

interface DropdownProps {
  entries: { value: string; label: string }[];
  onSelect: (value: string) => void;
  onRemove: (value: string) => void;
  visible: boolean;
  onClose: () => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
}

function RecentDropdown({ entries, onSelect, onRemove, visible, onClose, inputRef }: DropdownProps) {
  const dropdownRef = useRef<HTMLDivElement>(null);
  const isInteracting = useRef(false);

  useEffect(() => {
    if (!visible) return;
    const handleClick = (e: MouseEvent) => {
      if (
        dropdownRef.current && !dropdownRef.current.contains(e.target as Node) &&
        inputRef.current && !inputRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [visible, onClose, inputRef]);

  if (!visible || entries.length === 0) return null;

  return (
    <div
      className="recent-dropdown"
      ref={dropdownRef}
      onMouseDown={() => { isInteracting.current = true; }}
      onMouseUp={() => { isInteracting.current = false; }}
    >
      <div className="recent-header">
        <span>Recent</span>
      </div>
      {entries.map((entry) => (
        <div
          key={entry.value}
          className="recent-item"
          onMouseDown={(e) => {
            e.preventDefault();
            onSelect(entry.value);
          }}
        >
          <svg className="recent-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <polyline points="12 6 12 12 16 14" />
          </svg>
          <span className="recent-label" title={entry.value}>{entry.label}</span>
          <button
            className="recent-remove"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onRemove(entry.value);
            }}
            title="Remove"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}

export function JoinGameBar({ selectedAccountIds }: JoinGameBarProps) {
  const [placeIdInput, setPlaceIdInput] = useState('');
  const [psLinkInput, setPsLinkInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ type: 'success' | 'error' | 'info'; message: string } | null>(null);

  const [showPlaceHistory, setShowPlaceHistory] = useState(false);
  const [showPsHistory, setShowPsHistory] = useState(false);

  const placeInputRef = useRef<HTMLInputElement>(null);
  const psInputRef = useRef<HTMLInputElement>(null);

  const placeHistory = useRecentHistory('place');
  const psHistory = useRecentHistory('ps');
  const { presets, addPreset, updatePreset, removePreset } = usePresets();

  // 'list' = show presets, 'save' = show save form, 'edit' = edit existing, null = closed
  const [presetView, setPresetView] = useState<'list' | 'save' | 'edit' | null>(null);
  const [presetName, setPresetName] = useState('');
  const [editPlaceId, setEditPlaceId] = useState('');
  const [editPsLink, setEditPsLink] = useState('');
  const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
  const presetRef = useRef<HTMLDivElement>(null);
  const presetBtnRef = useRef<HTMLButtonElement>(null);

  const handleJoin = async () => {
    if (selectedAccountIds.length === 0) {
      setStatus({ type: 'error', message: 'Select account(s) first' });
      setTimeout(() => setStatus(null), 3000);
      return;
    }

    setLoading(true);
    setStatus(null);

    try {
      let placeId = parsePlaceId(placeIdInput);
      let linkCode = parseLinkCode(psLinkInput);

      // If Place ID field is empty, try to get it from PS link
      if (!placeId && psLinkInput.trim()) {
        // Old format PS links contain Place ID in the URL path
        placeId = parsePlaceId(psLinkInput);
      }

      // If still no Place ID and we have a share link, resolve it via backend
      if (!placeId && psLinkInput.trim()) {
        try {
          setStatus({ type: 'info', message: 'Resolving share link...' });
          const [resolvedPlaceId, resolvedCode] = await resolveShareLink(psLinkInput.trim());
          placeId = resolvedPlaceId;
          if (!linkCode) linkCode = resolvedCode;
          setStatus(null);
        } catch (err) {
          console.error('Failed to resolve share link:', err);
          setStatus({ type: 'error', message: 'Failed to resolve PS link. Enter Place ID manually.' });
          setLoading(false);
          setTimeout(() => setStatus(null), 4000);
          return;
        }
      }

      if (!placeId) {
        setStatus({ type: 'error', message: 'Enter a Place ID or a game URL' });
        setLoading(false);
        setTimeout(() => setStatus(null), 3000);
        return;
      }

      // Save to recent history on successful parse
      if (placeIdInput.trim()) {
        placeHistory.addEntry(placeIdInput.trim(), shortenLabel(placeIdInput.trim()));
      }
      if (psLinkInput.trim()) {
        psHistory.addEntry(psLinkInput.trim(), shortenLabel(psLinkInput.trim()));
      }

      // If joining a private server, try to pre-resolve the access code once.
      // The result is cached server-side so subsequent joins reuse it.
      // If pre-resolve fails, we still proceed — join_game will try with just linkCode.
      if (linkCode) {
        setStatus({ type: 'info', message: 'Resolving private server...' });
        for (const aid of selectedAccountIds) {
          try {
            await resolvePrivateServer(aid, placeId, linkCode);
            break; // Cached server-side, one success is enough
          } catch (err) {
            console.warn(`[RAM] resolvePrivateServer failed for account ${aid}:`, err);
          }
        }
      }

      let successCount = 0;
      let failCount = 0;
      let lastError = '';

      for (let i = 0; i < selectedAccountIds.length; i++) {
        const accountId = selectedAccountIds[i];
        try {
          // Small delay between launches to avoid rate limiting
          if (i > 0) {
            await new Promise(r => setTimeout(r, 1500));
          }
          setStatus({ type: 'info', message: `Launching ${i + 1}/${selectedAccountIds.length}...` });
          await joinGame(
            accountId,
            placeId,
            undefined, // jobId
            undefined, // followUserId
            linkCode || undefined,
            undefined, // launchArgs
          );
          successCount++;
        } catch (err) {
          console.error(`Failed to join game for ${accountId}:`, err);
          lastError = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
          failCount++;
        }
      }

      if (failCount === 0) {
        setStatus({ type: 'success', message: `Launched ${successCount} instance${successCount > 1 ? 's' : ''}` });
      } else if (successCount === 0) {
        setStatus({ type: 'error', message: lastError || 'All failed' });
      } else {
        setStatus({ type: 'error', message: `${successCount} ok, ${failCount} failed: ${lastError}` });
      }
      setTimeout(() => setStatus(null), 8000);
    } catch (err) {
      console.error('Join error:', err);
      setStatus({ type: 'error', message: typeof err === 'string' ? err : 'Join failed' });
      setTimeout(() => setStatus(null), 4000);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      setShowPlaceHistory(false);
      setShowPsHistory(false);
      handleJoin();
    }
    if (e.key === 'Escape') {
      setShowPlaceHistory(false);
      setShowPsHistory(false);
      setPresetView(null);
    }
  };

  // Close preset dropdown on outside click
  useEffect(() => {
    if (!presetView) return;
    const handleClick = (e: MouseEvent) => {
      if (
        presetRef.current && !presetRef.current.contains(e.target as Node) &&
        presetBtnRef.current && !presetBtnRef.current.contains(e.target as Node)
      ) {
        setPresetView(null);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [presetView]);

  const handleSavePreset = async () => {
    const name = presetName.trim();
    if (!name) return;
    await addPreset(name, placeIdInput.trim(), psLinkInput.trim());
    setPresetName('');
    setPresetView(null);
  };

  const openEditPreset = (preset: { id: string; name: string; place_id: string; ps_link: string }) => {
    setEditingPresetId(preset.id);
    setPresetName(preset.name);
    setEditPlaceId(preset.place_id);
    setEditPsLink(preset.ps_link);
    setPresetView('edit');
  };

  const handleUpdatePreset = async () => {
    const name = presetName.trim();
    if (!name || !editingPresetId) return;
    await updatePreset(editingPresetId, name, editPlaceId.trim(), editPsLink.trim());
    setPresetName('');
    setEditPlaceId('');
    setEditPsLink('');
    setEditingPresetId(null);
    setPresetView(null);
  };

  const handleLoadPreset = (preset: { place_id: string; ps_link: string }) => {
    setPlaceIdInput(preset.place_id);
    setPsLinkInput(preset.ps_link);
    setPresetView(null);
  };

  return (
    <div className="join-game-bar">
      <div className="join-field">
        <label>Place ID / URL</label>
        <div className="join-input-wrapper">
          <input
            ref={placeInputRef}
            type="text"
            className="input-field join-input join-input-with-arrow"
            placeholder="1818 or roblox.com/games/..."
            value={placeIdInput}
            onChange={e => setPlaceIdInput(e.target.value)}
            onFocus={() => setShowPlaceHistory(true)}
            onBlur={() => { setTimeout(() => setShowPlaceHistory(false), 150); }}
            onKeyDown={handleKeyDown}
            disabled={loading}
          />
          {placeHistory.entries.length > 0 && (
            <button
              type="button"
              className="recent-arrow-btn"
              tabIndex={-1}
              onMouseDown={(e) => {
                e.preventDefault();
                setShowPlaceHistory(prev => !prev);
              }}
              title="Recent"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                <path d="M2 3.5L5 7L8 3.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </button>
          )}
          <RecentDropdown
            entries={placeHistory.entries}
            onSelect={(val) => { setPlaceIdInput(val); setShowPlaceHistory(false); }}
            onRemove={(val) => placeHistory.removeEntry(val)}
            visible={showPlaceHistory}
            onClose={() => setShowPlaceHistory(false)}
            inputRef={placeInputRef}
          />
        </div>
      </div>
      <div className="join-field">
        <label>Private Server Link</label>
        <div className="join-input-wrapper">
          <input
            ref={psInputRef}
            type="text"
            className="input-field join-input join-input-with-arrow"
            placeholder="PS link (optional)"
            value={psLinkInput}
            onChange={e => setPsLinkInput(e.target.value)}
            onFocus={() => setShowPsHistory(true)}
            onBlur={() => { setTimeout(() => setShowPsHistory(false), 150); }}
            onKeyDown={handleKeyDown}
            disabled={loading}
          />
          {psHistory.entries.length > 0 && (
            <button
              type="button"
              className="recent-arrow-btn"
              tabIndex={-1}
              onMouseDown={(e) => {
                e.preventDefault();
                setShowPsHistory(prev => !prev);
              }}
              title="Recent"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                <path d="M2 3.5L5 7L8 3.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </button>
          )}
          <RecentDropdown
            entries={psHistory.entries}
            onSelect={(val) => { setPsLinkInput(val); setShowPsHistory(false); }}
            onRemove={(val) => psHistory.removeEntry(val)}
            visible={showPsHistory}
            onClose={() => setShowPsHistory(false)}
            inputRef={psInputRef}
          />
        </div>
      </div>
      <div className="preset-container" ref={presetRef}>
        <button
          ref={presetBtnRef}
          type="button"
          className="btn btn-ghost preset-btn"
          title="Presets"
          onMouseDown={(e) => {
            e.preventDefault();
            setPresetView(prev => prev ? null : 'list');
          }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"></path>
          </svg>
        </button>
        {presetView === 'list' && (
          <div className="preset-dropdown">
            <div className="preset-dropdown-header">
              <span>Presets</span>
              <button
                className="preset-add-btn"
                onMouseDown={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setPresetView('save');
                }}
                title="Save current as preset"
              >+</button>
            </div>
            {presets.length === 0 ? (
              <div className="preset-empty">No presets yet</div>
            ) : (
              presets.map((p) => (
                <div
                  key={p.id}
                  className="preset-item"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    handleLoadPreset(p);
                  }}
                >
                  <svg className="preset-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"></path>
                  </svg>
                  <span className="preset-label" title={`${p.place_id} | ${p.ps_link}`}>{p.name}</span>
                  <button
                    className="preset-edit-btn"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      openEditPreset(p);
                    }}
                    title="Edit"
                  >
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button
                    className="recent-remove"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      removePreset(p.id);
                    }}
                    title="Remove"
                  >×</button>
                </div>
              ))
            )}
          </div>
        )}
        
      </div>
      <button
        className="btn btn-primary join-btn"
        onClick={handleJoin}
        disabled={loading || (!placeIdInput.trim() && !psLinkInput.trim())}
      >
        {loading ? (
          <svg className="spin" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="12" y1="2" x2="12" y2="6"></line>
            <line x1="12" y1="18" x2="12" y2="22"></line>
            <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
            <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
            <line x1="2" y1="12" x2="6" y2="12"></line>
            <line x1="18" y1="12" x2="22" y2="12"></line>
          </svg>
        ) : (
          <>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polygon points="5 3 19 12 5 21 5 3"></polygon>
            </svg>
            Join{selectedAccountIds.length > 1 ? ` (${selectedAccountIds.length})` : ''}
          </>
        )}
      </button>
      {status && (
        <span className={`join-status ${status.type}`}>{status.message}</span>
      )}
      {presetView === 'save' && (
        <div className="dialog-overlay" onMouseDown={() => setPresetView(null)}>
          <div className="dialog preset-save-dialog" onMouseDown={(e) => e.stopPropagation()}>
            <div className="dialog-header">
              <h2>Save Preset</h2>
              <button className="dialog-close" onClick={() => setPresetView(null)}>
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div className="dialog-body">
              <div className="form-group">
                <label className="form-label">Preset Name</label>
                <input
                  type="text"
                  className="input-field"
                  placeholder="e.g. Grows Offline PS"
                  value={presetName}
                  onChange={(e) => setPresetName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') { e.preventDefault(); handleSavePreset(); }
                    if (e.key === 'Escape') { setPresetView(null); }
                  }}
                  autoFocus
                />
              </div>
              <div className="form-group">
                <label className="form-label">Place ID</label>
                <div className="preset-save-value">{placeIdInput.trim() || '(empty)'}</div>
              </div>
              <div className="form-group">
                <label className="form-label">PS Link</label>
                <div className="preset-save-value">{psLinkInput.trim() || '(empty)'}</div>
              </div>
            </div>
            <div className="dialog-footer">
              <button className="btn btn-ghost" onClick={() => setPresetView(null)}>Cancel</button>
              <button
                className="btn btn-primary"
                onClick={handleSavePreset}
                disabled={!presetName.trim()}
              >Save Preset</button>
            </div>
          </div>
        </div>
      )}
      {presetView === 'edit' && (
        <div className="dialog-overlay" onMouseDown={() => setPresetView(null)}>
          <div className="dialog preset-save-dialog" onMouseDown={(e) => e.stopPropagation()}>
            <div className="dialog-header">
              <h2>Edit Preset</h2>
              <button className="dialog-close" onClick={() => setPresetView(null)}>
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div className="dialog-body">
              <div className="form-group">
                <label className="form-label">Preset Name</label>
                <input
                  type="text"
                  className="input-field"
                  placeholder="Preset name"
                  value={presetName}
                  onChange={(e) => setPresetName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') { e.preventDefault(); handleUpdatePreset(); }
                    if (e.key === 'Escape') { setPresetView(null); }
                  }}
                  autoFocus
                />
              </div>
              <div className="form-group">
                <label className="form-label">Place ID</label>
                <input
                  type="text"
                  className="input-field"
                  placeholder="Place ID or URL"
                  value={editPlaceId}
                  onChange={(e) => setEditPlaceId(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') { e.preventDefault(); handleUpdatePreset(); }
                    if (e.key === 'Escape') { setPresetView(null); }
                  }}
                />
              </div>
              <div className="form-group">
                <label className="form-label">PS Link</label>
                <input
                  type="text"
                  className="input-field"
                  placeholder="Private server link (optional)"
                  value={editPsLink}
                  onChange={(e) => setEditPsLink(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') { e.preventDefault(); handleUpdatePreset(); }
                    if (e.key === 'Escape') { setPresetView(null); }
                  }}
                />
              </div>
            </div>
            <div className="dialog-footer">
              <button className="btn btn-ghost" onClick={() => setPresetView(null)}>Cancel</button>
              <button
                className="btn btn-primary"
                onClick={handleUpdatePreset}
                disabled={!presetName.trim()}
              >Update</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
