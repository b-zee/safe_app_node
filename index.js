const assert = require('assert');
const safe = require('./native/index.node');

const APP_NAME = 'app_from_js';

safe.test_create_app(APP_NAME, (err, app) => {
	assert(err === null);
	assert(app instanceof ArrayBuffer);
	
	safe.app_pub_enc_key(app, (err, key) => {
		assert(key instanceof ArrayBuffer);
	
		safe.enc_pub_key_get(app, key, (err, key) => {
			assert(key instanceof ArrayBuffer);
		});
	});

	safe.app_container_name(APP_NAME, (err, name) => {
		assert(name === 'apps/' + APP_NAME);
	});
});

