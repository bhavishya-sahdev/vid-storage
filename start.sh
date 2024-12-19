#!/bin/bash

GREEN='\033[0;32m'
NC='\033[0m'

cleanup() {
    echo -e "\n${GREEN}Shutting down services${NC}"
    pkill -f "cargo run"
    pkill -f "npm run dev"
    kill $(jobs -p) 2>/dev/null || true 
    exit 0
}

trap cleanup SIGINT

echo -e "${GREEN}Starting Rust backend${NC}"
cargo run &

echo -e "${GREEN}Starting Remix frontend${NC}"
cd frontend && npm run dev &

cd - > /dev/null

wait