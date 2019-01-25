function assert(b) { if (!b) { throw new Error(); } }

const safe = require('./native/index.node');

safe.test_create_app("app_from_js", (err, app) => {
    assert(err === null);
    assert(app instanceof ArrayBuffer);

    // safe.app_pub_enc_key(app, (...args) => {
    //     console.dir(args);
    // });
});

