package com.example.mmc

import android.Manifest
import android.os.Bundle
import android.widget.Button
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "MmcDemo"
        private const val PERMISSION_REQUEST_CODE = 100
    }

    private lateinit var statusText: TextView
    private lateinit var startDiscoveryButton: Button
    private lateinit var startMirrorButton: Button
    private lateinit var sendFileButton: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText = findViewById(R.id.statusText)
        startDiscoveryButton = findViewById(R.id.startDiscoveryButton)
        startMirrorButton = findViewById(R.id.startMirrorButton)
        sendFileButton = findViewById(R.id.sendFileButton)

        updateStatus("Ready")

        startDiscoveryButton.setOnClickListener {
            updateStatus("Starting discovery...")
            // TODO: Integrate with mmc-core
            Toast.makeText(this, "Discovery started", Toast.LENGTH_SHORT).show()
        }

        startMirrorButton.setOnClickListener {
            updateStatus("Starting mirror session...")
            // TODO: Integrate with mmc-core
            Toast.makeText(this, "Mirror session started", Toast.LENGTH_SHORT).show()
        }

        sendFileButton.setOnClickListener {
            updateStatus("Sending file...")
            // TODO: Integrate with mmc-core
            Toast.makeText(this, "File transfer initiated", Toast.LENGTH_SHORT).show()
        }
    }

    private fun updateStatus(message: String) {
        statusText.text = message
    }
}
