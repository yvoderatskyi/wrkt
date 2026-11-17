#!/usr/bin/env bash
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
cargo build --quiet
wrkt="$PWD/target/debug/wrkt"

sandbox=$(mktemp -d)
trap 'rm -rf "$sandbox"' EXIT
unset $(env | grep -o '^GIT_[A-Z_]*') XDG_CONFIG_HOME
export HOME="$sandbox/home" GIT_CONFIG_GLOBAL="$sandbox/gitconfig"
mkdir -p "$HOME/code"
git config --global init.defaultBranch main
git config --global user.name demo
git config --global user.email demo@example.com

for repo in api web; do
  git init --quiet --bare "$sandbox/origin-$repo.git"
  git clone --quiet "$sandbox/origin-$repo.git" "$HOME/code/$repo" 2>/dev/null
  git -C "$HOME/code/$repo" commit --quiet --allow-empty -m init
  git -C "$HOME/code/$repo" push --quiet origin main
done

step() { echo; echo "\$ wrkt $*"; "$wrkt" "$@"; echo "(exit $?)"; }

step repo add ~/code/api
step repo add ~/code/web
step repo list
step new PROJ-123 api web
git -C ~/wrkt/PROJ-123/api commit --quiet --allow-empty -m change
cd ~/wrkt/PROJ-123
step status
step sync
step push
step status
step list
cd /
step rm PROJ-123
