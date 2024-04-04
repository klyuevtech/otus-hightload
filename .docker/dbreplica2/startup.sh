#!/bin/bash
set -e

rm -rf /var/lib/postgresql/data/*
pg_basebackup --host=db --username=postgres --pgdata=/var/lib/postgresql/data --wal-method=stream --write-recovery-conf
cp /usr/share/postgresql/postgresql.conf.sample /var/lib/postgresql/data/postgresql.conf
echo "hot_standby_feedback = on" >> /var/lib/postgresql/data/postgresql.conf
echo "max_connections = 500" >> /var/lib/postgresql/data/postgresql.conf
echo "max_standby_streaming_delay = -1" >> /var/lib/postgresql/data/postgresql.conf
