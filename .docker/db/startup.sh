#!/bin/bash
set -e

mkdir -p /tmp/archivedir
echo "listen_addresses = '*'" >> /var/lib/postgresql/data/postgresql.conf
echo "wal_level = replica" >> /var/lib/postgresql/data/postgresql.conf
echo "archive_mode = on" >> /var/lib/postgresql/data/postgresql.conf
echo "archive_command = 'test -f /tmp/archivedir/%f || cp %p /tmp/archivedir/%f'" >> /var/lib/postgresql/data/postgresql.conf
echo "host    replication     postgres             0.0.0.0/0            trust" >> /var/lib/postgresql/data/pg_hba.conf
