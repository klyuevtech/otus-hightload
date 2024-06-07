
-- sessions: BEGIN

box.schema.space.create('sessions', { if_not_exists = true })
box.space.sessions:create_index('primary', { type = "HASH", unique = true, parts = { 1, 'string' }, if_not_exists = true })

-- async fn get_by_id(&self, id: &str) -> Result<Session, Error>;
function session_create(id, user_id, data)
    return box.space.sessions:insert{id, user_id, data}
end

-- async fn create(&self, session: &Session) -> Result<Uuid, Error>;
function session_get_by_id(id)
    return box.space.sessions.index.primary:select(id)[1]
end

-- sessions: END


-- friends: BEGIN

box.schema.space.create('friends', { if_not_exists = true })
box.space.friends:create_index('primary', { type = "HASH", unique = true, parts = { 1, 'string' }, if_not_exists = true })
box.space.friends:create_index('user_id', { type = "BITSET", unique = false, parts = { 2, 'string' }, if_not_exists = true })
box.space.friends:create_index('friend_id', { type = "BITSET", unique = false, parts = { 3, 'string' }, if_not_exists = true })
box.space.friends:create_index('user_id_friend_id', { type = "HASH", unique = true, parts = { 2, 'string', 3, 'string' }, if_not_exists = true })

-- async fn get_by_id(&self, id: &Uuid) -> Result<Friend, Error>;
function friend_get_by_id(id)
    if box.space.friends.index.primary == nil then
        return nil
    end
    return box.space.friends.index.primary:select(id)[1]
end

-- async fn get_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Friend>, Error>;
function friend_get_by_user_id(user_id)
    if box.space.friends.index.user_id == nil then
        return {}
    end
    return box.space.friends.index.user_id:select(user_id)
end

-- async fn get_by_friend_id(&self, friend_id: &Uuid) -> Result<Vec<Friend>, Error>;
function friend_get_by_friend_id(friend_id)
    if box.space.friends.index.friend_id == nil then
        return {}
    end
    return box.space.friends.index.friend_id:select(friend_id)
end

-- async fn get_by_user_id_and_friend_id(&self, user_id: &Uuid, friend_id: &Uuid) -> Result<Friend, Error>;
function friend_get_by_user_id_and_friend_id(user_id, friend_id)
    if box.space.friends.index.user_id_friend_id == nil then
        return nil
    end
    return box.space.friends.index.user_id_friend_id:select(user_id, friend_id)[1]
end

-- async fn create(&self, friend: &Friend) -> Result<Uuid, Error>;
function friend_create(id, user_id, friend_id)
    return box.space.friends:insert{id, user_id, friend_id}
end

-- async fn delete(&self, friend: &Friend) -> Result<bool, Error>;
function friend_delete(id)
    if box.space.friends.index.primary == nil then
        return false
    end
    box.space.friends.index.primary:delete(id)
    return true
end

-- async fn is_persistant(&self, friend: &Friend) -> Result<bool, Error>;
function friend_is_persistant(id)
    if box.space.friends.index.primary == nil or box.space.friends.index.primary:select(id)[1] == nil then
        return false
    end
    return true
end

-- friends: END

function get_leader_name()
    return box.info.election.leader_name
end