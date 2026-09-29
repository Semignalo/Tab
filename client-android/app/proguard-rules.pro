# BouncyCastle dipakai lewat API ringan (org.bouncycastle.crypto.*); R8 menyusutkan sisanya.
-dontwarn org.bouncycastle.**
-keep class org.bouncycastle.crypto.digests.Blake2sDigest { *; }
-keep class org.bouncycastle.crypto.macs.HMac { *; }
-keep class org.bouncycastle.crypto.modes.ChaCha20Poly1305 { *; }
-keep class org.bouncycastle.math.ec.rfc7748.** { *; }
