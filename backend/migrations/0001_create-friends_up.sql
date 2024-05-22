CREATE TABLE IF NOT EXISTS friends (
  id UUID DEFAULT uuid_generate_v4(),
  user_id UUID,
  friend_id UUID
);
