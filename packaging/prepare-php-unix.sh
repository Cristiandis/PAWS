#!/usr/bin/env bash
# Arrange the PHP runtime files that packaging expects.
#
# Usage: packaging/prepare-php-unix.sh <php-artifact-dir> [php-fpm-path]
#
# <php-artifact-dir> is the directory extracted from a php.yml artifact. It must
# contain LICENSE (PHP's license) and optionally a licenses/ directory (extra
# licenses gathered by static-php-cli). The php-fpm binary is taken from the
# second argument when given, otherwise from <php-artifact-dir>/php-fpm.
#
# Creates:
#   php-runtime/php/php-fpm
#   php-runtime/php/php.ini
#   licenses/PAWS-LICENSE
#   licenses/PHP-LICENSE
#   licenses/<whatever the artifact ships>
set -euo pipefail

src="${1:?usage: prepare-php-unix.sh <php-artifact-dir> [php-fpm-path]}"
fpm="${2:-${src}/php-fpm}"

cd "$(dirname "$0")/.."

mkdir -p php-runtime/php licenses
install -m 755 "$fpm" php-runtime/php/php-fpm
install -m 644 packaging/php.ini php-runtime/php/php.ini
install -m 644 LICENSE licenses/PAWS-LICENSE
install -m 644 "$src/LICENSE" licenses/PHP-LICENSE
if [ -d "$src/licenses" ]; then
	cp -r "$src/licenses"/. licenses/
fi

echo "PHP runtime staged:"
ls -l php-runtime/php licenses
