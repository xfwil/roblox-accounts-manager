import React, { useState } from 'react';
import { getServers, getGameDetails, joinGame } from '../api/game';
import type { GameServer, GameDetails } from '../types/game';
import { parsePlaceId } from './JoinGameBar';
import './ServerBrowserPage.css';

interface ServerBrowserPageProps {
  selectedAccountIds: string[];
}

const errText = (err: unknown, fallback: string) =>
  typeof err === 'string' ? err : err instanceof Error ? err.message : fallback;

export function ServerBrowserPage({ selectedAccountIds }: ServerBrowserPageProps) {
  const [placeId, setPlaceId] = useState('');
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [joiningServerId, setJoiningServerId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const [gameDetails, setGameDetails] = useState<GameDetails | null>(null);
  const [servers, setServers] = useState<GameServer[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  // Place the current list belongs to; the input may be edited after loading
  const [loadedPlaceId, setLoadedPlaceId] = useState<number | null>(null);

  const count = selectedAccountIds.length;

  const loadInitial = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const parsedPlaceId = parsePlaceId(placeId);
    if (!parsedPlaceId) {
      setError('Enter a Place ID or game URL');
      return;
    }

    setLoading(true);
    setError(null);
    setStatus(null);
    setServers([]);
    setNextCursor(null);
    setGameDetails(null);
    setLoadedPlaceId(null);

    try {
      const [detailsResult, serversResult] = await Promise.all([
        getGameDetails(parsedPlaceId).catch(() => null),
        getServers(parsedPlaceId, undefined, 25),
      ]);
      if (detailsResult && detailsResult.length > 0) setGameDetails(detailsResult[0]);
      setLoadedPlaceId(parsedPlaceId);
      setServers(serversResult[0]);
      setNextCursor(serversResult[1]);
    } catch (err: unknown) {
      console.error('Failed to load servers', err);
      setError(errText(err, 'Failed to load servers'));
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
      setError(errText(err, 'Failed to load more servers'));
    } finally {
      setLoadingMore(false);
    }
  };

  // Same pacing as the launch panel: 1.5s between launches to avoid rate limits
  const handleJoin = async (jobId: string) => {
    if (!loadedPlaceId || count === 0) return;
    setJoiningServerId(jobId);
    setError(null);
    let ok = 0;
    let lastError = '';
    for (let i = 0; i < count; i++) {
      if (i > 0) await new Promise(r => setTimeout(r, 1500));
      setStatus(`Launching ${i + 1}/${count}...`);
      try {
        await joinGame(selectedAccountIds[i], loadedPlaceId, jobId);
        ok++;
      } catch (err) {
        console.error('Failed to join game', err);
        lastError = errText(err, 'Failed to launch game');
      }
    }
    setStatus(ok === count ? `Launched ${ok} instance${ok > 1 ? 's' : ''}` : null);
    if (ok < count) setError(`${ok} ok, ${count - ok} failed: ${lastError}`);
    setJoiningServerId(null);
  };

  const copyJobId = async (jobId: string) => {
    try {
      await navigator.clipboard.writeText(jobId);
      setCopiedId(jobId);
      setTimeout(() => setCopiedId(prev => (prev === jobId ? null : prev)), 1500);
    } catch {
      setError('Unable to copy Job ID');
    }
  };

  return (
    <div className="server-page">
      <section className="server-heading">
        <div>
          <h1>Server browser</h1>
          <p>Find an open spot and send your selected accounts into the same instance.</p>
        </div>
        <span className={`server-launching ${count ? '' : 'empty'}`}>
          {count ? `Joining with ${count} account${count > 1 ? 's' : ''}` : 'Select accounts in All accounts first'}
        </span>
      </section>

      <form onSubmit={loadInitial} className="server-search-form">
        <input
          type="text"
          className="input-field"
          placeholder="Place ID or roblox.com/games/... URL"
          value={placeId}
          onChange={e => setPlaceId(e.target.value)}
          disabled={loading}
        />
        <button type="submit" className="btn btn-primary" disabled={loading || !placeId.trim()}>
          {loading ? 'Loading...' : 'Load servers'}
        </button>
      </form>

      {error && <div className="alert-error">{error}</div>}
      {status && <div className="server-status">{status}</div>}

      {gameDetails && (
        <div className="game-details-card">
          <h3>{gameDetails.name}</h3>
          <span className="game-stat">{(gameDetails.playing || 0).toLocaleString()} playing</span>
          <span className="game-stat">{(gameDetails.visits || 0).toLocaleString()} visits</span>
        </div>
      )}

      <div className="server-list-container">
        {loading ? (
          <div className="server-list-empty">Loading servers...</div>
        ) : servers.length > 0 ? (
          <>
            <table className="server-table">
              <thead>
                <tr>
                  <th className="col-players">Players</th>
                  <th className="col-ping">Ping</th>
                  <th className="col-fps">FPS</th>
                  <th className="col-job">Job ID</th>
                  <th className="col-action">Action</th>
                </tr>
              </thead>
              <tbody>
                {servers.map(server => (
                  <tr key={server.id}>
                    <td>
                      <div className="player-count">
                        <span className="current">{server.playing}</span>
                        <span className="max">/ {server.maxPlayers}</span>
                      </div>
                      <div className="player-bar">
                        <div className="player-bar-fill" style={{ width: `${Math.min(100, (server.playing / server.maxPlayers) * 100)}%` }} />
                      </div>
                    </td>
                    <td>
                      <span className={`ping-badge ${server.ping === null ? 'ping-ok' : server.ping < 80 ? 'ping-good' : server.ping < 150 ? 'ping-ok' : 'ping-bad'}`}>
                        {server.ping === null ? 'N/A' : `${server.ping} ms`}
                      </span>
                    </td>
                    <td>{server.fps.toFixed(1)}</td>
                    <td><code className="job-id" title={server.id}>{server.id.substring(0, 8)}…</code></td>
                    <td className="col-action">
                      <button className="btn btn-ghost btn-sm" onClick={() => copyJobId(server.id)} title="Copy Job ID">
                        {copiedId === server.id ? 'Copied' : 'Copy ID'}
                      </button>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => handleJoin(server.id)}
                        disabled={joiningServerId !== null || count === 0}
                        title={count === 0 ? 'Select accounts in All accounts first' : 'Join this server'}
                      >
                        {joiningServerId === server.id ? 'Joining...' : `Join${count > 1 ? ` (${count})` : ''}`}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {nextCursor && (
              <div className="load-more-container">
                <button className="btn btn-secondary" onClick={loadMore} disabled={loadingMore}>
                  {loadingMore ? 'Loading...' : 'Load more servers'}
                </button>
              </div>
            )}
          </>
        ) : (
          <div className="server-list-empty">
            {loadedPlaceId ? 'No servers found.' : 'Enter a Place ID to browse public servers.'}
          </div>
        )}
      </div>
    </div>
  );
}
