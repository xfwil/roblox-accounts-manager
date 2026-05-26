import React, { useState, useRef, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { importCookies, importUserPassCookie, exportAccounts } from '../api/importExport';
import { importLegacyAccounts } from '../api/importLegacy';
import type { ImportResult, LegacyImportResult } from '../types/index';
import './ImportExportDialog.css';

interface ImportExportDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onImportComplete: () => void;
}

export function ImportExportDialog({ isOpen, onClose, onImportComplete }: ImportExportDialogProps) {
  const [activeTab, setActiveTab] = useState<'import' | 'export' | 'legacy'>('import');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Import State
  const [importText, setImportText] = useState('');
  const [importGroup, setImportGroup] = useState('');
  const [importFormat, setImportFormat] = useState<'cookies' | 'userpass'>('cookies');
  const [importResults, setImportResults] = useState<ImportResult[] | null>(null);

  // Export State
  const [exportFormat, setExportFormat] = useState<'cookies' | 'user:cookie' | 'json'>('user:cookie');
  const [exportText, setExportText] = useState('');
  const [copied, setCopied] = useState(false);

  // Legacy Import State
  const [legacyFileName, setLegacyFileName] = useState<string | null>(null);
  const [legacyFileContent, setLegacyFileContent] = useState<string | null>(null);
  const [legacyValidateOnline, setLegacyValidateOnline] = useState(true);
  const [legacyResults, setLegacyResults] = useState<LegacyImportResult[] | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Streaming progress state
  const [streamingResults, setStreamingResults] = useState<ImportResult[]>([]);
  const [streamingLegacyResults, setStreamingLegacyResults] = useState<LegacyImportResult[]>([]);
  const [progressCurrent, setProgressCurrent] = useState(0);
  const [progressTotal, setProgressTotal] = useState(0);

  // Listen for import-progress events (cookie import)
  useEffect(() => {
    if (!loading) return;

    const unlisten = listen<{ current: number; total: number; result: ImportResult }>(
      'import-progress',
      (event) => {
        setProgressCurrent(event.payload.current);
        setProgressTotal(event.payload.total);
        setStreamingResults(prev => [...prev, event.payload.result]);
      }
    );

    return () => { unlisten.then(fn => fn()); };
  }, [loading, activeTab]);

  // Listen for legacy-import-progress events
  useEffect(() => {
    if (!loading) return;

    const unlisten = listen<{ current: number; total: number; result: LegacyImportResult }>(
      'legacy-import-progress',
      (event) => {
        setProgressCurrent(event.payload.current);
        setProgressTotal(event.payload.total);
        setStreamingLegacyResults(prev => [...prev, event.payload.result]);
      }
    );

    return () => { unlisten.then(fn => fn()); };
  }, [loading, activeTab]);

  if (!isOpen) return null;

  const handleImport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!importText.trim()) {
      setError('Please paste data to import');
      return;
    }

    setLoading(true);
    setError(null);
    setImportResults(null);
    setStreamingResults([]);
    setProgressCurrent(0);
    setProgressTotal(0);

    try {
      // Split by newlines and clean empty lines
      const entries = importText.split('\n').map(l => l.trim()).filter(l => l.length > 0);
      setProgressTotal(entries.length);
      
      let results: ImportResult[];
      if (importFormat === 'cookies') {
        results = await importCookies(entries, importGroup || undefined);
      } else {
        results = await importUserPassCookie(entries, importGroup || undefined);
      }
      
      setImportResults(results);
      setStreamingResults([]);
      onImportComplete();
    } catch (err: unknown) {
      console.error('Import failed', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Import failed');
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    setCopied(false);

    try {
      const data = await exportAccounts(exportFormat);
      setExportText(data);
    } catch (err: unknown) {
      console.error('Export failed', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Export failed');
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = () => {
    if (!exportText) return;
    navigator.clipboard.writeText(exportText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setLegacyFileName(file.name);
    setError(null);

    const reader = new FileReader();
    reader.onload = (ev) => {
      const content = ev.target?.result;
      if (typeof content === 'string') {
        setLegacyFileContent(content);
      }
    };
    reader.onerror = () => {
      setError('Failed to read file');
    };
    reader.readAsText(file);
  };

  const handleLegacyImport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!legacyFileContent) {
      setError('Please select an AccountData.json file first');
      return;
    }

    setLoading(true);
    setError(null);
    setLegacyResults(null);
    setStreamingLegacyResults([]);
    setProgressCurrent(0);
    setProgressTotal(0);

    try {
      const results = await importLegacyAccounts(legacyFileContent, legacyValidateOnline);
      setLegacyResults(results);
      setStreamingLegacyResults([]);
      onImportComplete();
    } catch (err: unknown) {
      console.error('Legacy import failed', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Legacy import failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog-content import-export-dialog" onClick={e => e.stopPropagation()}>
        <div className="dialog-header">
          <div className="ie-tabs">
            <button 
              className={`ie-tab ${activeTab === 'import' ? 'active' : ''}`}
              onClick={() => setActiveTab('import')}
            >
              Import Accounts
            </button>
            <button 
              className={`ie-tab ${activeTab === 'export' ? 'active' : ''}`}
              onClick={() => setActiveTab('export')}
            >
              Export Accounts
            </button>
            <button 
              className={`ie-tab ${activeTab === 'legacy' ? 'active' : ''}`}
              onClick={() => setActiveTab('legacy')}
            >
              Legacy Import
            </button>
          </div>
          <button className="btn-icon" onClick={onClose}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <div className="dialog-body ie-body">
          {error && <div className="alert-error ie-alert">{error}</div>}

          {activeTab === 'import' && (
            <form onSubmit={handleImport} className="ie-form">
              {!importResults ? (
                <>
                  <div className="form-group">
                    <label>Format</label>
                    <div className="radio-group">
                      <label className="radio-label">
                        <input 
                          type="radio" 
                          name="importFormat" 
                          checked={importFormat === 'cookies'} 
                          onChange={() => setImportFormat('cookies')} 
                        />
                        Cookies Only
                      </label>
                      <label className="radio-label">
                        <input 
                          type="radio" 
                          name="importFormat" 
                          checked={importFormat === 'userpass'} 
                          onChange={() => setImportFormat('userpass')} 
                        />
                        User:Pass:Cookie
                      </label>
                    </div>
                  </div>

                  <div className="form-group">
                    <label>Group <span className="optional">(optional)</span></label>
                    <input
                      type="text"
                      className="input-field"
                      placeholder="Assign to group"
                      value={importGroup}
                      onChange={e => setImportGroup(e.target.value)}
                    />
                  </div>

                  <div className="form-group flex-1">
                    <label>Data (One per line)</label>
                    <textarea
                      className="input-field ie-textarea"
                      placeholder={importFormat === 'cookies' ? "ROBLOSECURITY=..." : "username:password:ROBLOSECURITY=..."}
                      value={importText}
                      onChange={e => setImportText(e.target.value)}
                    />
                  </div>

                  <div className="dialog-footer ie-footer">
                    <button type="submit" className="btn btn-primary" disabled={loading || !importText.trim()}>
                      {loading
                        ? `Importing... ${progressCurrent}/${progressTotal}`
                        : 'Import Accounts'}
                    </button>
                  </div>

                  {/* Live streaming progress during import */}
                  {loading && streamingResults.length > 0 && (
                    <div className="streaming-progress">
                      <div className="progress-bar-container">
                        <div
                          className="progress-bar-fill"
                          style={{ width: `${progressTotal > 0 ? (progressCurrent / progressTotal) * 100 : 0}%` }}
                        />
                      </div>
                      <div className="streaming-results-list">
                        {streamingResults.map((result, i) => (
                          <div key={i} className={`stream-item ${result.success ? 'success' : 'fail'}`}>
                            <span className="stream-dot" />
                            {result.success
                              ? result.username || 'Success'
                              : result.error || 'Failed'}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <div className="import-results">
                  <h3>Import Results</h3>
                  <div className="results-summary">
                    <span className="success-count">{importResults.filter(r => r.success).length} Succeeded</span>
                    <span className="fail-count">{importResults.filter(r => !r.success).length} Failed</span>
                  </div>
                  <div className="results-list">
                    {importResults.map((result, i) => (
                      <div key={i} className={`result-item ${result.success ? 'success' : 'fail'}`}>
                        {result.success ? (
                          <>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                              <polyline points="22 4 12 14.01 9 11.01"></polyline>
                            </svg>
                            {result.username || 'Success'}
                          </>
                        ) : (
                          <>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <circle cx="12" cy="12" r="10"></circle>
                              <line x1="12" y1="8" x2="12" y2="12"></line>
                              <line x1="12" y1="16" x2="12.01" y2="16"></line>
                            </svg>
                            {result.error || 'Failed'}
                          </>
                        )}
                      </div>
                    ))}
                  </div>
                  <div className="dialog-footer ie-footer">
                    <button type="button" className="btn btn-secondary" onClick={() => setImportResults(null)}>
                      Back
                    </button>
                    <button type="button" className="btn btn-primary" onClick={onClose}>
                      Done
                    </button>
                  </div>
                </div>
              )}
            </form>
          )}

          {activeTab === 'export' && (
            <form onSubmit={handleExport} className="ie-form">
              <div className="form-group">
                <label>Export Format</label>
                <div className="radio-group">
                  <label className="radio-label">
                    <input 
                      type="radio" 
                      name="exportFormat" 
                      checked={exportFormat === 'user:cookie'} 
                      onChange={() => setExportFormat('user:cookie')} 
                    />
                    User:Cookie
                  </label>
                  <label className="radio-label">
                    <input 
                      type="radio" 
                      name="exportFormat" 
                      checked={exportFormat === 'cookies'} 
                      onChange={() => setExportFormat('cookies')} 
                    />
                    Cookies Only
                  </label>
                  <label className="radio-label">
                    <input 
                      type="radio" 
                      name="exportFormat" 
                      checked={exportFormat === 'json'} 
                      onChange={() => setExportFormat('json')} 
                    />
                    JSON
                  </label>
                </div>
              </div>

              <div className="dialog-footer ie-footer no-border">
                <button type="submit" className="btn btn-primary" disabled={loading}>
                  {loading ? 'Generating...' : 'Generate Export'}
                </button>
              </div>

              {exportText && (
                <div className="export-result">
                  <div className="export-header">
                    <label>Exported Data</label>
                    <button type="button" className="btn btn-secondary btn-sm" onClick={copyToClipboard}>
                      {copied ? 'Copied!' : 'Copy to Clipboard'}
                    </button>
                  </div>
                  <textarea
                    className="input-field ie-textarea readonly"
                    value={exportText}
                    readOnly
                  />
                </div>
              )}
            </form>
          )}

          {activeTab === 'legacy' && (
            <form onSubmit={handleLegacyImport} className="ie-form">
              {!legacyResults ? (
                <>
                  <div className="legacy-info">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <circle cx="12" cy="12" r="10"></circle>
                      <line x1="12" y1="16" x2="12" y2="12"></line>
                      <line x1="12" y1="8" x2="12.01" y2="8"></line>
                    </svg>
                    <span>Import accounts from the original Roblox Account Manager (C#) AccountData.json file. DPAPI-encrypted cookies cannot be imported — export with "No Encryption" from the original app first.</span>
                  </div>

                  <div className="form-group">
                    <label>AccountData.json File</label>
                    <input
                      ref={fileInputRef}
                      type="file"
                      accept=".json"
                      onChange={handleFileSelect}
                      className="file-input-hidden"
                    />
                    <div className="file-picker-row">
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => fileInputRef.current?.click()}
                      >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                          <polyline points="17 8 12 3 7 8"></polyline>
                          <line x1="12" y1="3" x2="12" y2="15"></line>
                        </svg>
                        Select File
                      </button>
                      {legacyFileName && (
                        <span className="file-name">{legacyFileName}</span>
                      )}
                    </div>
                  </div>

                  <div className="form-group">
                    <label className="checkbox-label">
                      <input
                        type="checkbox"
                        checked={legacyValidateOnline}
                        onChange={(e) => setLegacyValidateOnline(e.target.checked)}
                      />
                      Validate cookies online
                    </label>
                    {!legacyValidateOnline && (
                      <div className="checkbox-warning">
                        Cookies will not be verified. Invalid or expired cookies will still be imported.
                      </div>
                    )}
                  </div>

                  <div className="dialog-footer ie-footer">
                    <button type="submit" className="btn btn-primary" disabled={loading || !legacyFileContent}>
                      {loading
                        ? `Importing... ${progressCurrent}/${progressTotal}`
                        : 'Import from Legacy'}
                    </button>
                  </div>

                  {/* Live streaming progress during legacy import */}
                  {loading && streamingLegacyResults.length > 0 && (
                    <div className="streaming-progress">
                      <div className="progress-bar-container">
                        <div
                          className="progress-bar-fill"
                          style={{ width: `${progressTotal > 0 ? (progressCurrent / progressTotal) * 100 : 0}%` }}
                        />
                      </div>
                      <div className="streaming-results-list">
                        {streamingLegacyResults.map((result, i) => (
                          <div key={i} className={`stream-item ${result.success ? 'success' : 'fail'}`}>
                            <span className="stream-dot" />
                            {result.success
                              ? result.username || 'Success'
                              : `${result.username ? result.username + ': ' : ''}${result.error || 'Failed'}`}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <div className="import-results">
                  <h3>Legacy Import Results</h3>
                  <div className="results-summary">
                    <span className="success-count">{legacyResults.filter(r => r.success).length} Succeeded</span>
                    <span className="fail-count">{legacyResults.filter(r => !r.success).length} Failed</span>
                  </div>
                  <div className="results-list">
                    {legacyResults.map((result, i) => (
                      <div key={i} className={`result-item ${result.success ? 'success' : 'fail'}`}>
                        {result.success ? (
                          <>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                              <polyline points="22 4 12 14.01 9 11.01"></polyline>
                            </svg>
                            <span>{result.username || 'Success'}</span>
                            {result.validated && <span className="badge badge-validated">Validated</span>}
                          </>
                        ) : (
                          <>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <circle cx="12" cy="12" r="10"></circle>
                              <line x1="12" y1="8" x2="12" y2="12"></line>
                              <line x1="12" y1="16" x2="12.01" y2="16"></line>
                            </svg>
                            <span>{result.username ? `${result.username}: ` : ''}{result.error || 'Failed'}</span>
                          </>
                        )}
                      </div>
                    ))}
                  </div>
                  <div className="dialog-footer ie-footer">
                    <button type="button" className="btn btn-secondary" onClick={() => setLegacyResults(null)}>
                      Back
                    </button>
                    <button type="button" className="btn btn-primary" onClick={onClose}>
                      Done
                    </button>
                  </div>
                </div>
              )}
            </form>
          )}
        </div>
      </div>
    </div>
  );
}
