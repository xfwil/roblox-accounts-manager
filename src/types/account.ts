export interface AccountView {
  id: string;
  username: string;
  user_id: number | null;
  display_name: string | null;
  description: string | null;
  robux: number | null;
  is_premium: boolean | null;
  avatar_url: string | null;
  group: string | null;
  alias: string | null;
  sort_order: number;
  last_used: string | null;
  created_at: string;
  fields: Record<string, string>;
}
