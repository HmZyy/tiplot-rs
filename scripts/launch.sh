#!/bin/bash

PROJECT_ROOT=~/projects/hmzyy/tiplot-rs
SCRIPTS_DIR="${PROJECT_ROOT}/scripts"
LOADER_DIR="${SCRIPTS_DIR}/loader"

VENV_DIR="${LOADER_DIR}/.venv"
PYTHON_BIN="${VENV_DIR}/bin/python3"

LOADER_SCRIPT="${LOADER_DIR}/main.py"
TIPLOT_BIN=~/.cargo/bin/tiplot

export TIPLOT_ASSETS_DIR="${PROJECT_ROOT}/assets"
export TIPLOT_LOADER_COMMAND="${PYTHON_BIN} ${LOADER_SCRIPT}"

exec "${TIPLOT_BIN}"
