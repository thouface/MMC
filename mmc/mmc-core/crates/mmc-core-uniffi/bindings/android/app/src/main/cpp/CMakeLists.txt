cmake_minimum_required(VERSION 3.18.1)

project("mmcnative")

# Add the native-lib.cpp implementation
add_library(
    mmcnative
    SHARED
    native-lib.cpp
)

# Find required system libraries
find_library(
    log-lib
    log
)

find_library(
    android-lib
    android
)

find_library(
    graphic-lib
    android
)

find_library(
    mincore-lib
    android
)

# Link libraries
target_link_libraries(
    mmcnative
    ${log-lib}
    ${android-lib}
    ${graphic-lib}
    ${mincore-lib}
)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
