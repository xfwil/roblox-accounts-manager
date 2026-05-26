import { useState, useEffect } from 'react';
import { getProcessStatus, findRobloxProcesses, cleanupDeadProcesses, untrackProcess } from '../api/watcher';
import type { ProcessStatus } from '../types/game';
import './WatcherPanel.css';

interface WatcherPanelProps {
  accounts: { id: string; username: string }[];
}

export function WatcherPanel({ accounts }: WatcherPanelProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [processes, setProcesses] = useState<ProcessStatus[]>([]);
  const [isRunningCount, setIsRunningCount] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Helper to map account ID to username
  const getAccountName = (id: string) => {
    const acc = accounts.find(a => a.id === id);
    return acc ? acc.username : id.substring(0, 8) + '...';
  };

  // Helper to format uptime (seconds) to readable string
  const formatUptime = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (mins < 60) return `${mins}m ${secs}s`;
    const hours = Math.floor(mins / 60);
    const remainingMins = mins % 60;
    return `${hours}h ${remainingMins}m`;
  };

  const fetchStatus = async () => {
    try {
      const stats = await getProcessStatus();
      setProcesses(stats);
      setIsRunningCount(stats.filter(p => p.is_running).length);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch watcher status:', err);
      // Don't show error continuously if it's just polling
    }
  };

  // Poll for status
  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 10000); // 10 seconds
    return () => clearInterval(interval);
  }, []);

  const handleScan = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsLoading(true);
    setError(null);
    try {
      await findRobloxProcesses();
      await fetchStatus();
    } catch (err: unknown) {
      setError('Scan failed');
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCleanup = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsLoading(true);
    try {
      await cleanupDeadProcesses();
      await fetchStatus();
    } catch (err) {
      console.error('Cleanup failed:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleUntrack = async (pid: number) => {
    try {
      await untrackProcess(pid);
      await fetchStatus();
    } catch (err) {
      console.error(`Untrack failed for PID ${pid}:`, err);
    }
  };

  return (
    <div className={`watcher-panel-container ${isExpanded ? 'expanded' : 'collapsed'}`}>
      <div className="watcher-header" onClick={() => setIsExpanded(!isExpanded)}>
        <div className="watcher-title">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--accent-primary)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
          </svg>
          <span>Process Watcher</span>
          <span className={`watcher-badge ${isRunningCount > 0 ? 'active' : ''}`}>
            {isRunningCount} Running
          </span>
        </div>
        
        <div className="watcher-controls">
          <div className="watcher-actions">
            <button 
              className="btn btn-icon" 
              onClick={handleScan}
              disabled={isLoading}
              title="Scan for processes"
            >
              <svg className={isLoading ? "spin" : ""} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
            </button>
            <button 
              className="btn btn-icon" 
              onClick={handleCleanup}
              disabled={isLoading}
              title="Cleanup dead processes"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="3 6 5 6 21 6"></polyline>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
              </svg>
            </button>
          </div>
          <div className="watcher-toggle">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="18 15 12 9 6 15"></polyline>
            </svg>
          </div>
        </div>
      </div>

      <div className="watcher-content">
        <div className="watcher-toolbar">
          <h3>Tracked Processes</h3>
          {error && <span className="watcher-error">{error}</span>}
        </div>

        <div className="watcher-table-container">
          {processes.length > 0 ? (
            <table className="watcher-table">
              <thead>
                <tr>
                  <th>Account</th>
                  <th>PID</th>
                  <th>Place ID</th>
                  <th>Status</th>
                  <th>Uptime</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {processes.map(proc => (
                  <tr key={proc.pid}>
                    <td>
                      <span title={proc.account_id}>{getAccountName(proc.account_id)}</span>
                    </td>
                    <td style={{ fontFamily: 'monospace' }}>{proc.pid}</td>
                    <td style={{ fontFamily: 'monospace' }}>{proc.place_id || 'Unknown'}</td>
                    <td>
                      <div className="status-indicator">
                        <div className={`status-dot ${proc.is_running ? 'running' : 'stopped'}`} />
                        {proc.is_running ? 'Running' : 'Stopped'}
                      </div>
                    </td>
                    <td>{proc.is_running ? formatUptime(proc.uptime_seconds) : '-'}</td>
                    <td>
                      <button 
                        className="btn btn-icon"
                        style={{ color: 'var(--text-muted)' }}
                        onClick={(e) => {
                          e.stopPropagation();
                          handleUntrack(proc.pid);
                        }}
                        title="Untrack Process"
                      >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <circle cx="12" cy="12" r="10"></circle>
                          <line x1="15" y1="9" x2="9" y2="15"></line>
                          <line x1="9" y1="9" x2="15" y2="15"></line>
                        </svg>
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div className="watcher-empty">
              No processes are currently being tracked.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
