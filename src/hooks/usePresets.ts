import { useState, useCallback, useEffect } from 'react';
import {
  getPresets,
  addPreset as apiAddPreset,
  updatePreset as apiUpdatePreset,
  removePreset as apiRemovePreset,
  type JoinPreset,
} from '../api/presets';

/**
 * Hook for managing join presets (named Place ID + PS Link combos).
 * Stored via Tauri backend in join_data.json.
 */
export function usePresets() {
  const [presets, setPresets] = useState<JoinPreset[]>([]);

  useEffect(() => {
    getPresets().then(setPresets).catch(console.error);
  }, []);

  const addPreset = useCallback(
    async (name: string, placeId: string, psLink: string) => {
      try {
        const updated = await apiAddPreset(name, placeId, psLink);
        setPresets(updated);
      } catch (err) {
        console.error('Failed to add preset:', err);
        throw err;
      }
    },
    [],
  );

  const updatePreset = useCallback(
    async (id: string, name: string, placeId: string, psLink: string) => {
      try {
        const updated = await apiUpdatePreset(id, name, placeId, psLink);
        setPresets(updated);
      } catch (err) {
        console.error('Failed to update preset:', err);
        throw err;
      }
    },
    [],
  );

  const removePreset = useCallback(
    async (id: string) => {
      try {
        const updated = await apiRemovePreset(id);
        setPresets(updated);
      } catch (err) {
        console.error('Failed to remove preset:', err);
        throw err;
      }
    },
    [],
  );

  return { presets, addPreset, updatePreset, removePreset };
}
