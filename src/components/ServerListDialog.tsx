import React, { useState } from 'react';
import { getServers, getGameDetails, joinGame } from '../api/game';
import { trackProcess } from '../api/watcher';
import type { GameServer, GameDetails } from '../types/game';
import './ServerListDialog.css';
import { Modal } from './Modal';

interface ServerListDialogProps {
  isOpen: boolean;
  onClose: () => void;
  accountId: string | null; // Null if opened globally, required to actually join
}

export function ServerListDialog({ isOpen, onClose, accountId }: ServerListDialogProps) {
  const [placeId, setPlaceId] = useState('');
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [joiningServerId, setJoiningServerId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [joinError, setJoinError] = useState<string | null>(null);
  
  const [gameDetails, setGameDetails] = useState<GameDetails | null>(null);
  const [servers, setServers] = useState<GameServer[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  // Place the current list belongs to; the input may be edited after loading
  const [loadedPlaceId, setLoadedPlaceId] = useState<number | null>(null);

  if (!isOpen) return null;

  const loadInitial = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    
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
    setJoinError(null);
    setServers([]);
    setNextCursor(null);
    setGameDetails(null);
    setLoadedPlaceId(null);

    try {
      // Load details and servers in parallel
      const [detailsResult, serversResult] = await Promise.all([
        getGameDetails(parsedPlaceId).catch(() => null),
        getServers(parsedPlaceId, undefined, 25)
      ]);

      if (detailsResult && detailsResult.length > 0) {
        setGameDetails(detailsResult[0]);
      }
      
      setLoadedPlaceId(parsedPlaceId);
      setServers(serversResult[0]);
      setNextCursor(serversResult[1]);
    } catch (err: unknown) {
      console.error('Failed to load servers', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Failed to load servers');
    } finally {
      setLoading(false);
    }
  };

  const loadMore = async () => {
    if (!nextCursor || !loadedPlaceId) return;

    setLoadingMore(true);
    try {
      const [newServers, newCursor] = await getServers(loadedPlaceId, nextCursor, 25);
      
      setServers(prev => [...prev, ...newServers]);
      setNextCursor(newCursor);
    } catch (err: unknown) {
      console.error('Failed to load more servers', err);
      setError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Failed to load more servers');
    } finally {
      setLoadingMore(false);
    }
  };

  const handleJoin = async (jobId: string) => {
    if (!accountId) {
      setJoinError('You must select an account before joining a server.');
      return;
    }

    if (!loadedPlaceId) return;
    setJoiningServerId(jobId);
    setJoinError(null);
    
    try {
      const pid = await joinGame(accountId, loadedPlaceId, jobId);
      trackProcess(pid, accountId, loadedPlaceId).catch(console.error);
      // Wait a moment before clearing status so user sees it succeeded
      setTimeout(() => {
        setJoiningServerId(null);
      }, 1000);
    } catch (err: unknown) {
      console.error('Failed to join game', err);
      setJoinError(typeof err === 'string' ? err : err instanceof Error ? err.message : 'Failed to launch game');
      setJoiningServerId(null);
    }
  };

  return (
    <Modal title="Server browser" description="Find an open spot. Choose the right instance for your session." onClose={onClose} className="server-dialog">

        <div className="dialog-body server-dialog-body">
          <form onSubmit={loadInitial} className="server-search-form">
            <div className="search-input-group">
              <input
                type="text"
                className="input-field"
                placeholder="Enter Place ID (e.g. 1818)"
                value={placeId}
                onChange={e => setPlaceId(e.target.value)}
                disabled={loading}
                autoFocus
              />
              <button type="submit" className="btn btn-primary" disabled={loading || !placeId}>
                {loading ? 'Loading...' : 'Load Servers'}
              </button>
            </div>
          </form>

          {error && (
            <div className="alert-error">
              {error}
            </div>
          )}

          {joinError && (
            <div className="alert-error">
              {joinError}
            </div>
          )}

          {gameDetails && (
            <div className="game-details-card">
              <div className="game-info-primary">
                <h3>{gameDetails.name}</h3>
                <span className="game-stat">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
                    <circle cx="9" cy="7" r="4"></circle>
                    <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
                    <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
                  </svg>
                  {(gameDetails.playing || 0).toLocaleString()} playing
                </span>
                <span className="game-stat">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
                  </svg>
                  {(gameDetails.visits || 0).toLocaleString()} visits
                </span>
              </div>
            </div>
          )}

          <div className="server-list-container">
            {loading ? (
              <div className="server-list-loading">
                <svg className="spin" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" strokeWidth="2">
                  <line x1="12" y1="2" x2="12" y2="6"></line>
                  <line x1="12" y1="18" x2="12" y2="22"></line>
                  <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
                  <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
                  <line x1="2" y1="12" x2="6" y2="12"></line>
                  <line x1="18" y1="12" x2="22" y2="12"></line>
                  <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"></line>
                  <line x1="16.24" y1="4.93" x2="19.07" y2="7.76"></line>
                </svg>
              </div>
            ) : servers.length > 0 ? (
              <>
                <table className="server-table">
                  <thead>
                    <tr>
                      <th className="col-players">Players</th>
                      <th className="col-ping">Ping</th>
                      <th className="col-fps">FPS</th>
                      <th className="col-action">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {servers.map((server) => (
                      <tr key={server.id}>
                        <td>
                          <div className="player-count">
                            <span className="current">{server.playing}</span>
                            <span className="max">/ {server.maxPlayers}</span>
                          </div>
                          <div className="player-bar">
                            <div 
                              className="player-bar-fill" 
                              style={{ width: `${Math.min(100, (server.playing / server.maxPlayers) * 100)}%` }}
                            ></div>
                          </div>
                        </td>
                        <td>
                          <span className={`ping-badge ${server.ping === null ? 'ping-ok' : server.ping < 80 ? 'ping-good' : server.ping < 150 ? 'ping-ok' : 'ping-bad'}`}>
                            {server.ping === null ? 'N/A' : `${server.ping} ms`}
                          </span>
                        </td>
                        <td>{server.fps.toFixed(1)}</td>
                        <td>
                          <button 
                            className="btn btn-secondary btn-sm" 
                            onClick={() => handleJoin(server.id)}
                            disabled={joiningServerId !== null || !accountId}
                            title={!accountId ? "Select an account from the main list first to join" : "Join this server"}
                          >
                            {joiningServerId === server.id ? 'Joining...' : 'Join'}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                
                {nextCursor && (
                  <div className="load-more-container">
                    <button 
                      className="btn btn-secondary" 
                      onClick={loadMore}
                      disabled={loadingMore}
                    >
                      {loadingMore ? 'Loading...' : 'Load More Servers'}
                    </button>
                  </div>
                )}
              </>
            ) : (
              !loading && !error && gameDetails && (
                <div className="server-list-empty">
                  No servers found.
                </div>
              )
            )}
          </div>
        </div>
    </Modal>
  );
}
