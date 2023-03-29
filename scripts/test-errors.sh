#!/bin/bash

# Script to trigger various error conditions.
# This can be used to visually verify that the error messages returned are meaningful.

VENV=venv
SERVER=http://localhost:8080
SOURCE=http://localhost:9000
USERNAME=minioadmin
PASSWORD=minioadmin
BUCKET=sample-data
OBJECT=data-uint32.dat
DTYPE=uint32

function expect_fail {
  python ./scripts/client.py "${@}"
  if [[ $? -eq 0 ]]; then
    echo "Request did not fail as expected"
    exit 1
  fi
}

set -e
source venv/bin/activate
set +e

echo "Invalid operation"
expect_fail notanoperation --server $SERVER --source notaurl --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Invalid source URL"
expect_fail select --server $SERVER --source notaurl --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Unreachable source"
expect_fail select --server $SERVER --source http://notasource.example.com --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Invalid source port"
expect_fail select --server $SERVER --source ${SOURCE::-1} --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Empty username"
expect_fail select --server $SERVER --source $SOURCE --username '' --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Invalid username"
expect_fail select --server $SERVER --source $SOURCE --username foo --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Empty password"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password '' --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Invalid password"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password foo --bucket $BUCKET --object $OBJECT --dtype $DTYPE
echo

echo "Empty bucket"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket '' --object $OBJECT --dtype $DTYPE
echo

echo "Invalid bucket"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket notabucket --object $OBJECT --dtype $DTYPE
echo

echo "Empty object"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object '' --dtype $DTYPE
echo

echo "Invalid object"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object notanobject --dtype $DTYPE
echo

echo "Invalid dtype"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype notadtype
echo

echo "Invalid shape type"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '{}'
echo

echo "Invalid shape index type"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '["notanumber"]'
echo

echo "Empty shape list"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[]'
echo

echo "Zero shape index"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[0]'
echo

echo "Invalid shape length"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[11]'
echo

echo "Selection without shape"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --selection '[[0, 1, 1]]'
echo

echo "Zero length selection"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[10]' --selection '[]'
echo

echo "Incompatible shape and selection"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[10]' --selection '[[0, 1, 1], [0, 1, 1]]'
echo

echo "Selection stride zero"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[10]' --selection '[[0, 1, 0]]'
echo

echo "Selection start at end"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[10]' --selection '[[1, 1, 0]]'
echo

echo "Zero size"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --size 0
echo

echo "Negative size"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --size -1
echo

echo "Size not multiple of dtype"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --size 3
echo

echo "Negative offset"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --offset -1
echo

echo "Invalid order"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --order notanorder
echo

echo "Empty selection"
expect_fail select --server $SERVER --source $SOURCE --username $USERNAME --password $PASSWORD --bucket $BUCKET --object $OBJECT --dtype $DTYPE --shape '[1]' --selection '[[1, 2, 1]]'
echo
