package com.example.mmc

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import android.util.Log

/**
 * Foreground Service for file transfer operations.
 *
 * This service keeps the app alive during large file transfers,
 * displaying a persistent notification to the user.
 */
class TransferForegroundService : Service() {

    companion object {
        private const val TAG = "TransferService"
        private const val CHANNEL_ID = "mmc_transfer_channel"
        private const val NOTIFICATION_ID = 1001

        const val ACTION_START = "com.example.mmc.action.START_TRANSFER"
        const val ACTION_STOP = "com.example.mmc.action.STOP_TRANSFER"
        const val EXTRA_FILE_NAME = "file_name"
        const val EXTRA_PROGRESS = "progress"
    }

    private var isRunning = false

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        Log.d(TAG, "TransferForegroundService created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                val fileName = intent.getStringExtra(EXTRA_FILE_NAME) ?: "Transfer"
                val progress = intent.getIntExtra(EXTRA_PROGRESS, 0)
                startForeground(NOTIFICATION_ID, createNotification(fileName, progress))
                isRunning = true
                Log.d(TAG, "Transfer service started for: $fileName")
            }
            ACTION_STOP -> {
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                isRunning = false
                Log.d(TAG, "Transfer service stopped")
            }
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        isRunning = false
        Log.d(TAG, "TransferForegroundService destroyed")
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val name = getString(R.string.notification_channel_name)
            val description = getString(R.string.notification_channel_description)
            val importance = NotificationManager.IMPORTANCE_LOW
            val channel = NotificationChannel(CHANNEL_ID, name, importance).apply {
                this.description = description
                setShowBadge(false)
            }
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(channel)
        }
    }

    private fun createNotification(fileName: String, progress: Int): Notification {
        val stopIntent = Intent(this, TransferForegroundService::class.java).apply {
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

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.foreground_notification_title))
            .setContentText("Transferring: $fileName")
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentIntent(mainPendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setProgress(100, progress, progress == 0)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Cancel", stopPendingIntent)

        return builder.build()
    }

    fun updateProgress(fileName: String, progress: Int) {
        if (isRunning) {
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.notify(NOTIFICATION_ID, createNotification(fileName, progress))
        }
    }
}
