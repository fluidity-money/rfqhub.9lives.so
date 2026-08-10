#!/bin/sh -e

table="rfqhub_migrations"

dbmate -d migrations --migrations-table "$table" -u "$1" up
