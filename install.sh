#!/bin/sh

err_msg="\033[31m> A problem occured during download!\033[0m"
add_path=false
version=latest

if [ -n "$1" ]; then
  version="$1"
fi
file_url="https://github.com/scnplt/crabd/releases/download/$version/linux-crabd.tar.gz"

read -p "Do you want to add crabd to /usr/bin/? (y/N): " user_input
if echo "$user_input" | grep -iq "^[Yy]$"; then
  add_path=true
fi

echo "Downloading from $file_url"

if which curl >/dev/null; then
  if ! curl -fSL "$file_url" -o linux-crabd.tar.gz; then
    echo "$err_msg"
    exit 1
  fi
elif which wget >/dev/null; then
  if ! wget "$file_url"; then
    echo "err_msg"
    exit 1
  fi
else
  echo 'curl or wget not installed!'
  exit 1
fi

tar xzf linux-crabd.tar.gz
rm linux-crabd.tar.gz

if $add_path; then
  echo '\033[33m> Adding crabd to /usr/bin/\033[0m'
  mv crabd /usr/bin && chmod +x /usr/bin/crabd
fi

echo '\033[32m> Installation complete.\033[0m'
