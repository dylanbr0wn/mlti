const { Binary } = require('binary-install');
const os = require('os');

function getPlatform() {
  const type = os.type();
  const arch = os.arch();

  if (type === 'Windows_NT' ){
    if (arch === 'x64') {
			return {platform:'win64', ext:'.exe'};
		} else {
			return {platform:'win32', ext:'.exe'};
		}
  }
  if (type === 'Linux' && arch === 'x64') return {platform:'linux64', ext:''};
  if (type === 'Darwin') return {platform:'darwin', ext:''};

  throw new Error(`Unsupported platform: ${type} ${arch}`);
}
function getBinary() {
  const platform = getPlatform();
  const version = require('../package.json').version;
  const url = `https://github.com/dylanbr0wn/mlti/releases/download/v${version}/mlti-${platform.platform}.tar.gz`;
  const name = `multi${platform.ext}`;
  return new Binary(name, url);
}

module.exports = getBinary;
