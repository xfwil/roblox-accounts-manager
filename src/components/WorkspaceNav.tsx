import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { Icon } from './Icon';

export function WorkspaceNav({ groups, selectedGroup, onGroup, onServers, onReports, onTransfer, onSettings, onMultiLogin, activeView, count }: {
  groups: string[]; selectedGroup: string; onGroup: (group: string) => void;
  onServers: () => void; onReports: () => void; onTransfer: () => void; onSettings: () => void; onMultiLogin: () => void;
  activeView: 'accounts' | 'multilogin' | 'servers' | 'reports'; count: number;
}) {
  const [version, setVersion] = useState('');
  useEffect(() => { getVersion().then(setVersion).catch(() => {}); }, []);
  return <aside className="workspace-nav">
    <div className="nav-brand"><span className="nav-logo" role="img" aria-label="RAM logo" /><div><strong>RAM</strong><span>Account manager{version && ` · v${version}`}</span></div></div>
    <nav aria-label="Workspace"><button aria-label="Reports" title="Reports" className={`nav-item ${activeView === 'reports' ? 'active' : ''}`} onClick={onReports}><Icon name="grid" /><span>Reports</span></button><button aria-label="All accounts" title="All accounts" className={`nav-item ${activeView === 'accounts' && selectedGroup === 'All' ? 'active' : ''}`} onClick={() => onGroup('All')}><Icon name="accounts" /><span>All accounts</span><b>{count}</b></button><button aria-label="Server browser" title="Server browser" className={`nav-item ${activeView === 'servers' ? 'active' : ''}`} onClick={onServers}><Icon name="server" /><span>Server browser</span></button><button aria-label="Import & export" title="Import & export" className="nav-item" onClick={onTransfer}><Icon name="transfer" /><span>Import & export</span></button><button aria-label="Multi-login" title="Multi-login" className={`nav-item ${activeView === 'multilogin' ? 'active' : ''}`} onClick={onMultiLogin}><Icon name="transfer" /><span>Multi-login</span></button></nav>
    <section className="nav-groups"><h2>Your groups <span>{groups.length}</span></h2>{groups.length ? groups.map(group => <button key={group} className={`nav-item ${activeView === 'accounts' && selectedGroup === group ? 'active' : ''}`} onClick={() => onGroup(group)}><Icon name="folder" size={16} /><span>{group}</span></button>) : <p>Organize accounts into groups from account actions.</p>}</section>
    <div className="nav-bottom"><div className="storage-note"><Icon name="shield" /><div><strong>Local workspace</strong><span>Account data stored locally.</span></div></div><button aria-label="Settings" title="Settings" className="nav-item" onClick={onSettings}><Icon name="settings" /><span>Settings</span></button></div>
  </aside>;
}
