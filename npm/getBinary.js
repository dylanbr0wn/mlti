const { Binary } = require('binary-install');
const os = require('os');

function getPlatform() {
  const type = os.type();
  const arch = os.arch();

  if (type === 'Windows_NT' ){
    if (arch === 'x64') {
			return 'win64';
		} else {
			return 'win32';
		}
  }
  if (type === 'Linux' && arch === 'x64') return 'linux';
  if (type === 'Darwin') return 'macos';

  throw new Error(`Unsupported platform: ${type} ${arch}`);
}
function getBinary() {
  const platform = getPlatform();
  const version = require('../package.json').version;
  const url = `https://github.com/username/my-program/releases/download/v${version}/my-program-${platform}.tar.gz`;
  const name = 'my-program';
  return new Binary(url, { name });
}

module.exports = getBinary;
