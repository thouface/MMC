package com.example.mmc

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat

/**
 * Foreground Service for screen capture (MediaProjection) operations.
 *
 * This service manages the MediaProjection virtual display
 * for screen mirroring.
 */
class ScreenCaptureService : Service() {

    companion object {
        private const val TAG = "ScreenCaptureService"
        private const val CHANNEL_ID = "mmc_screen_capture_channel"
        private const val NOTIFICATION_ID = 1002

        const val ACTION_START = "com.example.mmc.action.START_SCREEN_CAPTURE"
        const val ACTION_STOP = "com.example.mmc.action.STOP_SCREEN_CAPTURE"
        const val ACTION_SET_RESULT = "com.example.mmc.action.SET_PROJECTION_RESULT"
        const val EXTRA_RESULT_CODE = "result_code"
        const val EXTRA_RESULT_DATA = "result_data"

        private const val REQUEST_MEDIA_PROJECTION = 0xDEAD
    }

    private var isCapturing = false
    private var mediaProjectionManager: MediaProjectionManager? = null
    private var notificationManager: NotificationManager? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        mediaProjectionManager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        Log.d(TAG, "ScreenCaptureService created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                // Start the service in foreground, then request projection permission
                startForeground(NOTIFICATION_ID, createNotification())
                isCapturing = false
                Log.d(TAG, "Screen capture service started")
            }
            ACTION_SET_RESULT -> {
                val resultCode = intent.getIntExtra(EXTRA_RESULT_CODE, Activity.RESULT_CANCELED)
                val resultData: Intent? = intent.getParcelableExtra(EXTRA_RESULT_DATA)
                handleProjectionResult(resultCode, resultData)
            }
            ACTION_STOP -> {
                stopCapture()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                isCapturing = false
                Log.d(TAG, "Screen capture service stopped")
            }
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        stopCapture()
        Log.d(TAG, "ScreenCaptureService destroyed")
    }

    private fun handleProjectionResult(resultCode: Int, resultData: Intent?) {
        if (resultCode == Activity.RESULT_OK && resultData != null) {
            // Register the MediaProjection with the Rust core
            val mediaProjection = mediaProjectionManager?.getMediaProjection(resultCode, resultData)
            if (mediaProjection != null) {
                MediaCapture.getInstance().setPermissionResult(resultCode, resultData)
                isCapturing = true
                updateNotification()
                Log.d(TAG, "MediaProjection registered successfully")
            } else {
                Log.e(TAG, "Failed to create MediaProjection")
            }
        } else {
            Log.w(TAG, "MediaProjection permission denied")
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
    }

    private fun stopCapture() {
        isCapturing = false
        MediaCapture.getInstance().stopCapture()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val name = getString(R.string.notification_channel_name)
            val description = "Screen capture notification"
            val importance = NotificationManager.IMPORTANCE_LOW
            val channel = NotificationChannel(CHANNEL_ID, name, importance).apply {
                this.description = description
                setShowBadge(false)
            }
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(channel)
        }
    }

    private fun createNotification(): Notification {
        val stopIntent = Intent(this, ScreenCaptureService::class.java).apply {
            action = ACTION_STOP
        }
        val stopPendingIntent = PendingIntent.getService(
            this, 0, stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val mainIntent = Intent(this, MainActivity::class.java)
        val mainPendingIntent = PendingIntent.getActivity(
            this, 0, mainIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.screen_capture_notification_title))
            .setContentText(getString(R.string.screen_capture_notification_text))
            .setSmallIcon(android.R.drawable.ic_menu_camera)
            .setContentIntent(mainPendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Stop", stopPendingIntent)
            .build()
    }

    private fun updateNotification() {
        if (isCapturing) {
            notificationManager?.notify(NOTIFICATION_ID, createNotification())
        }
    }

    /**
     * Request MediaProjection permission.
     * Must be called from an Activity context.
     */
    fun requestScreenCapture(activity: Activity) {
        val intent = mediaProjectionManager?.createScreenCaptureIntent()
        intent?.let {
            activity.startActivityForResult(it, REQUEST_MEDIA_PROJECTION)
        }
    }
}
