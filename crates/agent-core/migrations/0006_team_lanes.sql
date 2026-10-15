CREATE TABLE team_lanes (
 root_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 lane_key TEXT NOT NULL,
 session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 PRIMARY KEY(root_id,lane_key)
);

-- One lead writes; all existing specialists become read-only, without paid Fast.
UPDATE sessions SET permission_profile='read_only', fast_mode=0 WHERE parent_session_id IS NOT NULL;
