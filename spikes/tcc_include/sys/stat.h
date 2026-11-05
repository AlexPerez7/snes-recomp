/* sys/stat.h mínima para TinyCC en un dispositivo sin cabeceras del sistema: solo stat() y los campos
 * st_size y st_mtime, con la disposición de la ABI de Linux de arm64 (bionic) y de x86_64. Código propio. */
#ifndef SNES_TCC_STAT_H
#define SNES_TCC_STAT_H
#include <stddef.h>
struct snes_tcc_timespec { long tv_sec; long tv_nsec; };
#if defined(__aarch64__)
struct stat {
    unsigned long st_dev, st_ino;
    unsigned int st_mode, st_nlink, st_uid, st_gid;
    unsigned long st_rdev, __pad1;
    long st_size;
    int st_blksize, __pad2;
    long st_blocks;
    struct snes_tcc_timespec st_atim, st_mtim, st_ctim;
    unsigned int __unused4, __unused5;
};
#elif defined(__x86_64__)
struct stat {
    unsigned long st_dev, st_ino, st_nlink;
    unsigned int st_mode, st_uid, st_gid, __pad0;
    unsigned long st_rdev;
    long st_size, st_blksize, st_blocks;
    struct snes_tcc_timespec st_atim, st_mtim, st_ctim;
    long __unused[3];
};
#else
#error "sys/stat.h de TinyCC: arquitectura sin disposición de struct stat"
#endif
#define st_mtime st_mtim.tv_sec
int stat(const char *path, struct stat *st);
#endif
