# RAM — Spesifikasi UI Saat Ini (untuk re-work)

> Snapshot lengkap UI frontend `ram-manager` pada commit `f287d40`.
> Tujuan: jadi bahan untuk menggambar ulang UI. Semua label UI ditulis persis seperti di aplikasi.
> Sumber: `src/` (React 19 + TypeScript + Vite), backend Tauri 2 (`src-tauri/`).

---

## 1. Ringkasan Aplikasi

**RAM (Roblox Account Manager)** — aplikasi desktop (Tauri) untuk:
- Menyimpan banyak akun Roblox (via cookie `.ROBLOSECURITY` atau login WebView).
- Melihat status akun (avatar, Robux, Premium, presence online/in-game).
- Meluncurkan Roblox untuk satu/banyak akun sekaligus ke Place ID / Private Server.
- Browse server publik sebuah game dan join ke server tertentu.
- Utilitas akun (ganti password/email/display name/deskripsi, lihat privacy).
- Import/Export akun, import dari RAM versi lama (C#).
- Memantau proses Roblox yang berjalan (Process Watcher).

**Jendela:** judul `Roblox Account Manager`, default 1600×900, minimum 900×500, resizable.
Desktop-feel: `user-select: none`, body tidak scroll (scroll di dalam komponen).

---

## 2. Layout Utama

Satu layar saja (tidak ada routing). Struktur vertikal (`.app-container`, flex column, 100vh):

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ TOOLBAR (60px)                                                                │
│ [+ Add Account] [→ Login] [⟳ Refresh All] [⇪ Import / Export] [🌐 Server     │
│  Browser]  "N Accounts"            [⏷ All Groups ▾] [🔍 Search accounts...] ⚙ │
├───────────────────────────────────────────────────────────────────────────────┤
│ JOIN GAME BAR                                                                 │
│ Place ID / URL            Private Server Link                                 │
│ [1818 or roblox.com/…  ▾] [PS link (optional)  ▾] [☰] [▶ Join (N)]  status…   │
├───────────────────────────────────────────────────────────────────────────────┤
│ ACCOUNT TABLE (flex:1, scroll)                                                │
│  ┌──┬──────────────┬─────────┬─────────┬──────────┬───────────┬────────────┐  │
│  │  │ USER ▲       │ ALIAS   │ GROUP   │ ROBUX    │ LAST USED │ DATE ADDED │  │
│  ├──┼──────────────┼─────────┼─────────┼──────────┼───────────┼────────────┤  │
│  │🖼●│ username     │ alias   │ [badge] │ R$ 1,234 │    3d ago │ Sep 1, 2026│  │
│  │  │ display name │         │         │ [P]      │  ⚠ 25d ago│            │  │
│  └──┴──────────────┴─────────┴─────────┴──────────┴───────────┴────────────┘  │
│                                                                               │
├───────────────────────────────────────────────────────────────────────────────┤
│ PROCESS WATCHER (fixed bottom, 48px collapsed / 350px expanded)               │
│ 📈 Process Watcher  [0 Running]                               [⟳] [🗑]  ⌃    │
└───────────────────────────────────────────────────────────────────────────────┘
```

Overlay: 1 context menu (klik kanan baris) + 8 dialog modal (lihat §5).

---

## 3. Design Tokens Saat Ini (`src/index.css`)

Tema gelap ungu. Tidak ada light mode yang benar-benar diterapkan (lihat §9).

| Token | Nilai | Pakai |
|---|---|---|
| `--bg-dark` | `#12121c` | background app, input |
| `--bg-panel` | `#1e1e2d` | toolbar, header tabel, dialog |
| `--bg-hover` | `#2a2a3c` | hover, badge |
| `--bg-selected` | `#3b2e5a` | baris terpilih |
| `--accent-primary` | `#7c3aed` | tombol utama, fokus |
| `--accent-hover` / `--accent-active` | `#8b5cf6` / `#6d28d9` | |
| `--text-primary/secondary/muted` | `#fff` / `#a1a1aa` / `#71717a` | |
| `--border-color` / `--border-light` | `#3f3f4e` / `rgba(255,255,255,.1)` | |
| `--error` / `--success` / `--warning` | `#ef4444` / `#10b981` / `#f59e0b` | |
| `--premium-color` | `#fbbf24` | badge Premium |
| Presence | offline `#71717a`, online `#00A2FF`, in-game `#02B757`, in-studio `#4629D8` | dot avatar |
| Font | `Inter` → system sans; mono `JetBrains Mono` → Consolas | (font tidak di-load, fallback ke system) |
| Spacing | 4 / 8 / 16 / 24 / 32 px | |
| Radius | 4 / 8 / 12 / full | |

**Komponen dasar:** `.btn` + `.btn-primary` (ungu solid) / `.btn-secondary` (panel + border) / `.btn-icon` (transparan) / `.btn-ghost` / `.btn-sm`; `.input-field`; `.badge`, `.badge-premium`; `.alert-error`, `.alert-success`; `.spin` (animasi putar ikon loading).
Ikon: SVG inline gaya Feather (stroke 2), tanpa library.

---

## 4. Komponen Layar Utama

### 4.1 Toolbar (`Toolbar.tsx`)
Kiri → kanan:

| Elemen | Tipe | Aksi |
|---|---|---|
| **Add Account** | primary, ikon `+` | buka *Add Account Dialog* |
| **Login** | primary, ikon log-in | buka jendela WebView login Roblox (backend). Setelah sukses, event `login-complete` → list di-refresh. Kalau akun sudah ada → di-update (upsert), bukan duplikat. |
| **Refresh All** | secondary, ikon refresh (berputar saat loading, tombol disabled) | refresh semua akun paralel dari API Roblox |
| **Import / Export** | secondary | buka *Import/Export Dialog* |
| **Server Browser** | secondary, ikon globe | buka *Server Browser Dialog* |
| `N Accounts` | teks muted | jumlah total akun (bukan hasil filter) |
| Filter grup | `<select>` 150px, ikon funnel | `All Groups` + daftar grup |
| Search | input 250px, ikon kaca pembesar | placeholder `Search accounts...`; cocokkan username, display name, alias (case-insensitive) |
| ⚙ | icon button | buka *Settings Dialog* |

### 4.2 Join Game Bar (`JoinGameBar.tsx`)
Bar "quick launch" untuk **akun yang sedang dipilih di tabel** (multi-select didukung).

Elemen:
1. **Place ID / URL** — input (h 32px), placeholder `1818 or roblox.com/games/...`. Menerima angka, URL `/games/<id>`, atau string berisi angka ≥4 digit.
2. **Private Server Link** — input, placeholder `PS link (optional)`. Menerima `?privateServerLinkCode=`, `/share?code=...&type=Server`, atau kode hex mentah.
3. Kedua input punya **dropdown Recent** (header `Recent`, item = label singkat + tombol `×` hapus). Muncul saat fokus atau klik panah ▾ (panah hanya tampil jika ada history). Disimpan di backend (file), bukan localStorage.
   - Label singkat: angka apa adanya; URL game → `123 — nama game`; share link → `Share: <12 char>…`; lainnya dipotong 40 char.
4. **Tombol Presets** (32×32, ghost, ikon list) → dropdown (min 200 / max 280px):
   - Header `Presets` + tombol `+` (*Save current as preset*).
   - Kosong: `No presets yet`.
   - Item: ikon, nama (tooltip `placeId | psLink`), tombol edit ✎, tombol `×` hapus. Klik item → isi kedua input.
5. **Join** — primary. Label `Join` atau `Join (N)` jika >1 akun dipilih. Spinner saat loading. Disabled jika kedua input kosong / sedang loading.
6. **Status text** (max 360px) — warna per tipe `success` / `error` / `info`, auto-hilang (3–8 detik).

Keyboard: `Enter` = Join, `Escape` = tutup semua dropdown.

**Alur Join:**
1. Tidak ada akun dipilih → error `Select account(s) first`.
2. Parse Place ID; jika kosong coba ambil dari PS link; jika masih kosong & ada share link → backend `resolve_share_link` (status `Resolving share link...`; gagal → `Failed to resolve PS link. Enter Place ID manually.`).
3. Tetap kosong → `Enter a Place ID or a game URL`.
4. Simpan input ke Recent.
5. Jika ada link code → `resolve_private_server` sekali (status `Resolving private server...`, di-cache backend).
6. Launch berurutan per akun, jeda **1.5 detik** antar akun, status `Launching i/N...`.
7. Hasil: `Launched N instance(s)` / pesan error terakhir / `X ok, Y failed: <err>`.

**Sub-dialog Save Preset** (`Save Preset`): input `Preset Name` (placeholder `e.g. Grows Offline PS`, autofocus), tampilan read-only `Place ID` & `PS Link` (atau `(empty)`). Tombol `Cancel` / `Save Preset`.
**Sub-dialog Edit Preset** (`Edit Preset`): input `Preset Name`, `Place ID` (`Place ID or URL`), `PS Link` (`Private server link (optional)`). Tombol `Cancel` / `Update`.

### 4.3 Account Table (`AccountList.tsx` + `AccountRow.tsx`)
Tabel full-width, header sticky, uppercase 11px. Semua kolom (kecuali avatar) bisa di-sort (klik header; ▲/▼ ungu). Default sort: `sort_order` asc (urutan manual).

| Kolom | Lebar | Isi |
|---|---|---|
| (avatar) | 50px | avatar 48×48 radius 8, fallback ikon user; **presence dot** 12px pojok kanan bawah (Offline/Online/In Game/In Studio, tooltip) |
| **User** | 20%, min 140 | username (bold) + display name (12px secondary) |
| **Alias** | 14% | italic; `-` muted jika kosong; bisa **edit inline** (input, Enter simpan, Esc batal, blur simpan) |
| **Group** | 14% | badge nama grup / `-` |
| **Robux** | 10%, mono | `R$ 1,234` hijau / `?` muted; badge kuning `P` jika Premium |
| **Last Used** | 12%, rata kanan | relatif: `Never`, `Just now`, `5m ago`, `3h ago`, `2d ago`, `4mo ago`, `1y ago`. **≥20 hari → warna kuning + ikon ⚠** (tooltip "Has not been used in 20+ days") |
| **Date Added** | 12%, rata kanan | tanggal lokal `Sep 1, 2026` |

**Seleksi:**
- Klik = pilih satu. `Ctrl/Cmd+klik` = toggle. `Shift+klik` = tambah ke seleksi (range belum diimplementasi).
- Klik area kosong = kosongkan seleksi.
- Klik kanan = pilih baris (jika belum) + buka context menu.
- Baris hover `--bg-hover`, terpilih `--bg-selected`.
- Seleksi dikirim ke Join Game Bar.

**State kosong/loading/error** (tengah area):
- Loading awal: spinner + `Loading accounts...`
- Error & list kosong: ikon ⓘ merah + `Failed to load accounts` + detail mono.
- Belum ada akun: ikon users + `No accounts found` / `Click "Add Account" to get started.`
- Filter tanpa hasil: ikon search + `No matching accounts` / `Try adjusting your filters or search query.`

**Presence:** polling tiap **2 menit** (`get_account_presences`).

### 4.4 Context Menu (klik kanan baris) (`AccountContextMenu.tsx`)
Posisi di kursor, di-clamp ke viewport. Tutup saat klik luar. Item (ikon + teks):

1. **Refresh** — refresh data akun ini
2. **Edit Alias** — aktifkan edit inline di kolom Alias
3. **Set Group** — buka *Set Group Dialog*
4. **Copy Username** — clipboard
5. **Copy Cookie** — ambil cookie dari backend → clipboard (gagal = diam, tanpa notifikasi)
6. **Join Game** — buka *Join Game Dialog* (advanced) untuk akun ini
7. **Account Utilities** — buka *Account Utilities Dialog*
8. **Open Browser** — buka jendela browser yang sudah login sebagai akun ini
9. — separator —
10. **Remove Account** (merah) — `window.confirm('Are you sure you want to remove this account?')` lalu hapus

> Catatan: context menu hanya bekerja untuk satu akun (tidak ada bulk action).

### 4.5 Process Watcher (`WatcherPanel.tsx`)
Panel **fixed di bawah**, collapsible (48px ↔ 350px, animasi). Klik header = toggle.

- Header: ikon activity ungu, `Process Watcher`, badge `N Running` (aktif jika >0), tombol ikon **Scan for processes** (⟳, cari proses Roblox) dan **Cleanup dead processes** (🗑), chevron toggle.
- Isi: judul `Tracked Processes` + error inline (`Scan failed`).
- Tabel: **Account** (username, tooltip id) · **PID** (mono) · **Place ID** (mono / `Unknown`) · **Status** (dot hijau `Running` / `Stopped`) · **Uptime** (`45s`, `3m 12s`, `2h 5m`, `-`) · **Actions** (tombol ✕ *Untrack Process*).
- Kosong: `No processes are currently being tracked.`
- Polling tiap **10 detik**.

---

## 5. Dialog

Semua modal: overlay gelap full-screen, klik overlay = tutup, header (judul + ✕), body, footer (tombol kanan).

### 5.1 Add Account (`AddAccountDialog.tsx`) — 500px
- Error box di atas (jika ada).
- **Roblox Security Cookie (.ROBLOSECURITY)** — textarea (min-h 100, placeholder `_|WARNING:-DO-NOT-SHARE-THIS...`, autofocus, wajib).
- Baris 2 kolom: **Alias (Optional)** (`Main, Alt 1, etc.`) | **Group (Optional)** (`General, Bots, etc.`, datalist grup yang ada).
- Footer: `Cancel` / `Add Account` → saat loading `⟳ Validating...` (cookie divalidasi ke Roblox).

### 5.2 Set Group (inline di `AccountList.tsx`) — 420px
- **Existing Groups** — chip: `No Group` + semua grup (chip aktif = ungu).
- **Or create new group** — input `New group name...` (autofocus, Enter = simpan).
- Footer: `Cancel` / `Save`.

### 5.3 Join Game (advanced, per akun) (`JoinGameDialog.tsx`)
- **Place ID \*** (`e.g. 1818`, autofocus, harus angka)
- **Job ID (Server Instance)** *(optional)* (`e.g. e84c4e09-...`)
- **Follow User ID** *(optional)* (`e.g. 123456`)
- **Extra Launch Args** *(optional)* (`--app`)
- Alert error / success `Game launched successfully!` (tutup otomatis 1.5 dtk).
- Footer: `Cancel` / `Join Game` → `Launching...`

### 5.4 Server Browser (`ServerListDialog.tsx`) — max 700px, tinggi 80vh (max 700)
- Form: input `Enter Place ID (e.g. 1818)` + tombol `Load Servers` / `Loading...`.
- Alert error load & error join.
- **Game card:** nama game, `N playing`, `N visits`.
- Tabel server (header sticky): **Players** (`12 / 20` + progress bar 4px) · **Ping** (badge: <80 hijau, <150 kuning, ≥150 merah, `N/A`) · **FPS** (1 desimal) · **Action** (`Join` / `Joining...`, secondary sm).
- Tombol Join disabled jika tidak ada akun aktif (tooltip `Select an account from the main list first to join`).
- `Load More Servers` (paginasi cursor, 20 per halaman).
- Kosong: `No servers found.`

### 5.5 Account Utilities (`AccountUtilsDialog.tsx`)
Judul: `Account Utilities: <username>`. Layout **tab vertikal kiri + konten kanan**.

| Tab | Isi | Tombol |
|---|---|---|
| **Password** | Current Password, New Password, Confirm New Password (validasi cocok) | `Change Password` / `Changing...` |
| **Email** | Current Password, New Email Address | `Change Email` / `Changing...` |
| **Display Name** | New Display Name (max 20) + hint `Costs 1,000 Robux if changed within the last 7 days.` | `Set Display Name` / `Saving...` |
| **Description** | textarea 6 baris (max 1000) | `Save Description` / `Saving...` |
| **Privacy** | read-only list label → nilai: Phone Discovery · Who can message me · Who can chat with me in app · Who can chat with me in game · Who can see my inventory · Who can trade with me · Trade Quality Filter (`Unknown` jika null) | — |

Alert success/error di atas konten. Field password dikosongkan setelah sukses.

### 5.6 Import / Export (`ImportExportDialog.tsx`)
Header berisi **3 tab**: `Import Accounts` · `Export Accounts` · `Legacy Import`.

**Import Accounts**
- **Format** radio: `Cookies Only` | `User:Pass:Cookie`
- **Group** *(optional)* — `Assign to group`
- **Data (One per line)** — textarea besar (placeholder sesuai format)
- `Import Accounts` → `Importing... x/y`
- Saat berjalan: **progress bar** + list streaming (dot hijau/merah, username atau error) — dari event `import-progress`.
- Setelah selesai → layar **Import Results**: `N Succeeded` / `N Failed`, list per item; tombol `Back` / `Done`.

**Export Accounts**
- **Export Format** radio: `User:Cookie` (default) | `Cookies Only` | `JSON`
- `Generate Export` / `Generating...`
- Hasil: label `Exported Data` + `Copy to Clipboard` (→ `Copied!` 2 dtk) + textarea read-only.

**Legacy Import**
- Info box: import dari `AccountData.json` RAM C# asli; cookie terenkripsi DPAPI tidak bisa — export dengan "No Encryption" dulu.
- **AccountData.json File** — tombol `Select File` + nama file.
- Checkbox **Validate cookies online** (default on). Jika off → warning `Cookies will not be verified. Invalid or expired cookies will still be imported.`
- `Import from Legacy` → `Importing... x/y` + progress streaming (event `legacy-import-progress`).
- Hasil: `Legacy Import Results`, item sukses bisa punya badge `Validated`.

### 5.7 Settings (`SettingsDialog.tsx`)
- **API Port** (number 1024–65535)
- **Nexus Port** (number 1024–65535)
- **Theme** (select: `dark`, `light`, `midnight`, `cyberpunk`)
- **Auto Refresh (Minutes, 0 to disable)**
- ☐ **Enable Watcher / Process Monitor**
- **Watcher Interval (Seconds)** (min 5, disabled jika watcher off)
- Pesan error/success; sukses → tutup otomatis 1.5 dtk.
- Footer: `Cancel` / `Save Settings` → `Saving...`
- Loading: `Loading settings...`; gagal: `Failed to load settings. Make sure the backend is running.`

---

## 6. Model Data (yang ditampilkan UI)

```ts
AccountView {
  id: string; username: string; user_id: number|null;
  display_name, description, avatar_url, group, alias: string|null;
  robux: number|null; is_premium: boolean|null;
  sort_order: number; last_used: string|null; created_at: string;
  fields: Record<string,string>;          // belum dipakai di UI
}
UserPresence { userId; presenceType /*0 off,1 online,2 game,3 studio*/; lastLocation; placeId; gameId; universeId; lastOnline }
GameServer   { id; maxPlayers; playing; playerTokens[]; fps; ping|null }
GameDetails  { placeId; name; description; builder; builderId; maxPlayers; playing; visits; favoritesCount; imageToken }
ProcessStatus{ account_id; pid; place_id; is_running; started_at; uptime_seconds }
AppSettings  { api_port; nexus_port; theme; auto_refresh_minutes; watcher_enabled; watcher_interval_seconds }
PrivacySettings { phoneDiscovery; privateMessagePrivacy; appChatPrivacy; gameChatPrivacy; inventoryPrivacy; tradePrivacy; tradeValue }
JoinPreset   { id; name; place_id; ps_link; created_at }
RecentEntry  { value; label; timestamp }
ImportResult { index; success; username|null; error|null }   // Legacy: + validated
```

Data yang **tersedia tapi belum ditampilkan**: `description` (hanya di Utilities), `fields`, `user_id`, `lastLocation`/`placeId` presence (bisa jadi "sedang main game X"), `GameDetails.builder/maxPlayers/favoritesCount/imageToken` (thumbnail game), `ProcessStatus.started_at`.

---

## 7. Backend Commands (Tauri `invoke`)

| Area | Command | Dipakai UI? |
|---|---|---|
| Akun | `list_accounts`, `add_account`, `remove_account`, `update_account`, `refresh_account`, `get_groups` | ✅ |
| | `reorder_accounts` | ⚠ ada di hook, **tidak ada UI** (drag & drop) |
| | `get_account`, `get_account_count` | ❌ |
| Game | `join_game`, `get_servers`, `get_game_details`, `resolve_private_server`, `resolve_share_link` | ✅ |
| | `get_roblox_path` | ❌ |
| Utilitas | `change_password`, `change_email`, `get_privacy_settings`, `set_account_description`, `set_display_name` | ✅ |
| | `get_gender`, `get_birthdate` | ❌ |
| Import/Export | `import_cookies`, `import_user_pass_cookie`, `export_accounts`, `get_account_cookie` | ✅ |
| | `update_account_cookie` | ❌ (bisa jadi "Update Cookie" di menu) |
| | `import_legacy_accounts` | ✅ |
| Watcher | `get_process_status`, `cleanup_dead_processes`, `find_roblox_processes`, `untrack_process` | ✅ |
| | `track_process`, `get_running_count` | ❌ |
| Settings | `get_settings`, `save_settings`, `get_themes` | ✅ |
| Presets/Recent | `get/add/remove_recent_place_id(s)`, `get/add/remove_recent_ps_link(s)`, `get/add/update/remove_preset` | ✅ |
| Login | `open_login_window`, `open_browser` | ✅ |
| Presence | `get_account_presences` | ✅ |
| | `get_single_presence` | ❌ |

**Events backend → UI:** `login-complete {success, account, error}`, `import-progress {current,total,result}`, `legacy-import-progress {current,total,result}`.

**Polling:** presence 2 menit · watcher 10 detik.

---

## 8. Alur Pengguna Utama

1. **Tambah akun** — Login (WebView) *atau* Add Account (paste cookie) *atau* Import (bulk).
2. **Organisasi** — alias inline, Set Group, filter grup, search, sort kolom.
3. **Launch cepat** — pilih 1..N baris → isi Place ID / PS link (atau pilih Recent/Preset) → Join.
4. **Launch spesifik** — klik kanan → Join Game (Job ID / Follow User) *atau* Server Browser → pilih server.
5. **Kelola akun** — klik kanan → Account Utilities / Copy Cookie / Open Browser / Remove.
6. **Monitor** — Process Watcher di bawah.
7. **Backup** — Import/Export.

---

## 9. Masalah / Gap yang Perlu Diperhatikan Saat Re-work

UX:
- **Tidak ada toast/notifikasi global.** Copy Cookie/Username, gagal login, refresh gagal → diam atau hanya `console.error`.
- **Error akun** (`useAccounts.error`) hanya tampil jika list kosong; kalau list ada isinya, error hilang begitu saja.
- **Remove** pakai `window.confirm` native.
- **Tidak ada bulk action** di context menu (refresh/remove/set group untuk banyak akun).
- **Shift+klik** tidak memilih range.
- **Reorder manual** (`sort_order`) ada di backend, tapi tidak ada drag & drop.
- **Server Browser** dari toolbar memakai `selectedAccountId` yang hanya di-set lewat context menu → sering tidak ada akun aktif, tombol Join disabled. Tidak terhubung dengan seleksi tabel.
- **Theme** di Settings tidak diterapkan di frontend sama sekali (hanya disimpan).
- **Counter** `N Accounts` menghitung total, bukan hasil filter / jumlah terpilih.
- Tidak ada indikator akun yang cookie-nya invalid/expired.
- Process Watcher `fixed` menutupi bagian bawah tabel (diakali `margin-bottom: 48px`).
- `index.html` title masih `ram-manager`; font Inter/JetBrains Mono tidak di-load.
- Style dialog tersebar di banyak file CSS (`.dialog`, `.dialog-content`, `.dialog-close` vs `.btn-icon`) → tidak konsisten.

Bug:
- **`JoinGameDialog`**: `Extra Launch Args` dikirim ke posisi argumen `linkCode` pada `joinGame(...)` (argumen ke-5), jadi launch args sebenarnya tidak terkirim.
- **Set Group → `No Group`** mengirim `group: undefined` → backend (`update_account`) tidak mengubah apa-apa; grup hanya terhapus jika dikirim string kosong `""`. Jadi "No Group" tidak berfungsi.
