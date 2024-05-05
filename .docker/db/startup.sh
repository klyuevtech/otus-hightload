#!/bin/bash

echo "listen_addresses = '*'" >> /var/lib/postgresql/data/postgresql.conf
echo "wal_level = replica" >> /var/lib/postgresql/data/postgresql.conf
echo "host    replication             replicator             0.0.0.0/0          trust" >> /var/lib/postgresql/data/pg_hba.conf