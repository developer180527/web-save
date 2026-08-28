package com.venugopal.websave_app

import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Enqueued as unique work with KEEP, so asking on every launch is
        // cheap and does not disturb an already-scheduled run.
        LinkCheckWorker.schedule(applicationContext)
    }
}
