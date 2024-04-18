#!/bin/bash
set -e

echo "listen_addresses = '*'" >> /var/lib/postgresql/data/postgresql.conf
echo "wal_level = replica" >> /var/lib/postgresql/data/postgresql.conf
echo "host    replication     postgres             0.0.0.0/0            trust" >> /var/lib/postgresql/data/pg_hba.conf
