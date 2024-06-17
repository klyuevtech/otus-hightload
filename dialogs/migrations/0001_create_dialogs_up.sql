CREATE TABLE IF NOT EXISTS dialogs (
    id UUID DEFAULT uuid_generate_v4(),
    user_id1 UUID,
    user_id2 UUID
);