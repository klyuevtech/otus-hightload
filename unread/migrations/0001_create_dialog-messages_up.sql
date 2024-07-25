CREATE TABLE IF NOT EXISTS dialog_messages (
    id UUID DEFAULT uuid_generate_v4(),
    dialog_id UUID DEFAULT uuid_generate_v4(),
    user_id UUID DEFAULT uuid_generate_v4(),
    time_created TIMESTAMP DEFAULT now(),
    content TEXT,
    constraint pk_dialog_messages primary key (id, dialog_id)
);