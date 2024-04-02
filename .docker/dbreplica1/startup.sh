#!/bin/bash
set -e

rm -rf /var/lib/postgresql/data/replica/*
pg_basebackup --host=db --username=postgres --pgdata=/var/lib/postgresql/data/replica --wal-method=stream --write-recovery-conf
cp /usr/share/postgresql/postgresql.conf.sample /var/lib/postgresql/data/replica/postgresql.conf
echo "hot_standby_feedback = on" >> /var/lib/postgresql/data/replica/postgresql.conf
echo "max_connections = 500" >> /var/lib/postgresql/data/replica/postgresql.conf
echo "max_standby_streaming_delay = -1" >> /var/lib/postgresql/data/replica/postgresql.conf
mkdir -p /tmp/archivedir
echo "restore_command = 'cp /tmp/archivedir/%f %p'" >> /var/lib/postgresql/data/replica/postgresql.conf
echo "archive_cleanup_command = 'pg_archivecleanup /tmp/archivedir %r'" >> /var/lib/postgresql/data/replica/postgresql.conf
