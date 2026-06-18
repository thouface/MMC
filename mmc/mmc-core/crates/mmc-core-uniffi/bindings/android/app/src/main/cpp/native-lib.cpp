// JNI implementation for Android platform integration
// Implements native methods referenced by platform_android.rs Rust FFI declarations
// and called by MediaCapture.kt / InputService.kt Kotlin companion objects.

#include <jni.h>
#include <android/log.h>
#include <android/os/Bundle.h>
#include <android/view/Surface.h>
#include <android/graphics/Bitmap.h>
#include <android/graphics/BitmapFactory.h>
#include <string>

#define TAG "MmcNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, TAG, __VA_ARGS__)
#define LOGD(...) __android_log_print(ANDROID_LOG_DEBUG, TAG, __VA_ARGS__)

// Forward declaration of Kotlin class references
static jclass g_mediaCaptureClass = nullptr;
static jclass g_inputServiceClass = nullptr;
static JavaVM* g_jvm = nullptr;

// Cache JNI references on library load
JNIEXPORT jint JNICALL
JNI_OnLoad(JavaVM* vm, void* reserved) {
    g_jvm = vm;
    JNIEnv* env;
    if (vm->GetEnv((void**)&env, JNI_VERSION_1_6) != JNI_OK) {
        return JNI_ERR;
    }
    LOGI("MmcNative JNI_OnLoad called");
    return JNI_VERSION_1_6;
}

// ============================================================================
// MediaCapture native methods
// ============================================================================

JNIEXPORT jboolean JNICALL
Java_com_example_mmc_MediaCapture_nativeRequestPermission(JNIEnv* env, jclass clazz) {
    // Permission is requested via startActivityForResult in Kotlin code.
    // This native method is a no-op that returns true as a placeholder.
    // The actual permission result is communicated via setPermissionResult().
    LOGI("nativeRequestPermission called");
    return JNI_TRUE;
}

JNIEXPORT jboolean JNICALL
Java_com_example_mmc_MediaCapture_nativeCaptureFrame(
        JNIEnv* env, jclass clazz,
        jbyteArray buffer, jint width, jint height) {

    if (buffer == nullptr || width <= 0 || height <= 0) {
        LOGE("nativeCaptureFrame: invalid parameters");
        return JNI_FALSE;
    }

    // Get buffer pointer
    jbyte* bufPtr = env->GetByteArrayElements(buffer, nullptr);
    if (bufPtr == nullptr) {
        LOGE("nativeCaptureFrame: failed to get buffer elements");
        return JNI_FALSE;
    }

    // In a real implementation, this would read from a Surface or
    // use AndroidGraphics::captureDisplay() API.
    // For now, fill with a test pattern (gradient).
    int totalPixels = width * height;
    int bufferSize = env->GetArrayLength(buffer);

    if (bufferSize < totalPixels * 4) {
        LOGE("nativeCaptureFrame: buffer too small (%d < %d)", bufferSize, totalPixels * 4);
        env->ReleaseByteArrayElements(buffer, bufPtr, 0);
        return JNI_FALSE;
    }

    // Generate a test gradient pattern
    uint8_t* rgba = reinterpret_cast<uint8_t*>(bufPtr);
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            int idx = (y * width + x) * 4;
            rgba[idx + 0] = static_cast<uint8_t>((x * 255) / width);     // R
            rgba[idx + 1] = static_cast<uint8_t>((y * 255) / height);    // G
            rgba[idx + 2] = 128;                                          // B
            rgba[idx + 3] = 255;                                          // A
        }
    }

    env->ReleaseByteArrayElements(buffer, bufPtr, 0);
    LOGD("nativeCaptureFrame: captured %dx%d frame", width, height);
    return JNI_TRUE;
}

// ============================================================================
// InputService native methods
// ============================================================================

JNIEXPORT jboolean JNICALL
Java_com_example_mmc_InputService_nativeEnable(JNIEnv* env, jclass clazz) {
    LOGI("nativeEnable called");
    // The actual enable is handled in Kotlin InputService.enableInjection()
    // This native method is a placeholder that confirms the library is loaded.
    return JNI_TRUE;
}

JNIEXPORT jboolean JNICALL
Java_com_example_mmc_InputService_nativeInjectTouch(
        JNIEnv* env, jclass clazz,
        jint touchType, jfloat x, jfloat y,
        jfloat pressure, jint pointerId, jlong sequenceId) {

    // Get the InputService Kotlin instance
    jmethodID getInstance = env->GetStaticMethodID(
        clazz, "getInstance", "()Lcom/example/mmc/InputService;");
    if (getInstance == nullptr) {
        LOGE("nativeInjectTouch: getInstance method not found");
        return JNI_FALSE;
    }

    jobject serviceInstance = env->CallStaticObjectMethod(clazz, getInstance);
    if (serviceInstance == nullptr) {
        LOGE("nativeInjectTouch: InputService not connected");
        return JNI_FALSE;
    }

    // Call the Kotlin dispatchTouchEvent method
    jmethodID dispatchTouch = env->GetMethodID(
        clazz, "dispatchTouchEvent",
        "(IFFFIJ)Z");
    if (dispatchTouch == nullptr) {
        LOGE("nativeInjectTouch: dispatchTouchEvent method not found");
        return JNI_FALSE;
    }

    jboolean result = env->CallBooleanMethod(
        serviceInstance, dispatchTouch,
        touchType, x, y, pressure, pointerId, sequenceId);

    return result;
}

JNIEXPORT jboolean JNICALL
Java_com_example_mmc_InputService_nativeInjectKey(
        JNIEnv* env, jclass clazz,
        jint keyType, jint keyCode, jlong sequenceId) {

    // Get the InputService Kotlin instance
    jmethodID getInstance = env->GetStaticMethodID(
        clazz, "getInstance", "()Lcom/example/mmc/InputService;");
    if (getInstance == nullptr) {
        LOGE("nativeInjectKey: getInstance method not found");
        return JNI_FALSE;
    }

    jobject serviceInstance = env->CallStaticObjectMethod(clazz, getInstance);
    if (serviceInstance == nullptr) {
        LOGE("nativeInjectKey: InputService not connected");
        return JNI_FALSE;
    }

    // Call the Kotlin dispatchKeyEvent method
    jmethodID dispatchKey = env->GetMethodID(
        clazz, "dispatchKeyEvent",
        "(IIJ)Z");
    if (dispatchKey == nullptr) {
        LOGE("nativeInjectKey: dispatchKeyEvent method not found");
        return JNI_FALSE;
    }

    jboolean result = env->CallBooleanMethod(
        serviceInstance, dispatchKey,
        keyType, keyCode, sequenceId);

    return result;
}
