box.schema.space.create('sessions', { if_not_exists = true })
box.space.sessions:create_index('primary', { type = "HASH", unique = true, parts = { 1, 'string' }, if_not_exists = true })

function session_create(id, user_id, data)
    return box.space.sessions:insert{id, user_id, data}
end

function session_get_by_id(id)
    return box.space.sessions.index.primary:select(id)[1]
end
