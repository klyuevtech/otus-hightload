CREATE TABLE IF NOT EXISTS dialog_messages (
    id UUID DEFAULT uuid_generate_v4(),
    sender_user_id UUID,
    receiver_user_id UUID,
    content TEXT,
    constraint pk_dialog_messages primary key (id)
);