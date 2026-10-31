/* play_state.h - the part of the interactive frontend that drives save states (snes_state.inc): slots on disk, rewind and the
 * notices. No SDL in here, so the headless frontend runs the same code for the tests (--state-script).
 *
 * Keys (play.c): F5 saves the current slot, F7 loads it, F3 and F4 change the slot (1 to 9), Backspace held rewinds. The state
 * of slot N lives in `<rom without extension>.sN.sst`, in the saves folder (--saves-dir) or next to the ROM. A state holds the memory of the game: it is never
 * published (.gitignore, `scan` and release.ps1 reject *.sst).
 *
 * The runtime only takes or loads a state at a safe pass through the resume point (rt_resume), so a request waits there;
 * play_state_tick sees it done (state_req back to 0) and finishes the work. Rewind keeps the newest state whole and, behind it,
 * the difference to each older one (XOR, runs of zeros skipped), so a long history costs little memory. */
#ifndef PLAY_STATE_H
#define PLAY_STATE_H

#define ST_SLOTS 9
#define ST_RING 150          /* captured states kept: at one every ST_EVERY frames, about 10 seconds */
#define ST_EVERY 4
#define ST_GIVE_UP_FRAMES 90 /* a request that has not found a safe point by then is dropped, with the reason */
#define ST_TOAST_FRAMES 120

static char st_rom_path[1100];
static int st_slot = 1;
static int st_pending;  /* 0 none, 1 save in flight, 2 load in flight, 3 capture for rewind in flight, 4 rewind load in flight */
static uint32_t st_since;

static char st_toast[96], st_toast2[96]; /* the notice, and an explanation under it (empty when there is none) */
static int st_toast_left;

static int st_echo; /* print the notices to the console too (the tests without a window read them) */

static void st_say(const char *text) {
    st_toast2[0] = 0;
    snprintf(st_toast, sizeof st_toast, "%s", text);
    st_toast_left = ST_TOAST_FRAMES;
    if (st_echo) printf("state: %s (frame %u)\n", text, (unsigned)frames);
}

/* --hash-wram: at exit, an FNV-1a of WRAM, so a test can tell two runs apart without a dump. */
static int st_hash_wram;
static void st_print_hash(void) {
    uint32_t h = 2166136261u;
    size_t i;
    for (i = 0; i < sizeof wram; i++) h = (h ^ wram[i]) * 16777619u;
    printf("wram: %08x\n", (unsigned)h);
}

/* ---- Rewind: the newest state whole, the older ones as differences ---- */

typedef struct { uint8_t *d; size_t n; } StDelta;
static uint8_t *st_full;      /* newest captured state (malloc), or the one rewound to */
static size_t st_full_len;
static StDelta st_ring[ST_RING];
static int st_head, st_count; /* st_ring[(st_head - 1 - k) % ST_RING] is the k-th delta back from the newest */
static int st_rewound;        /* a rewind is in progress: the first step loads st_full itself */
static uint32_t st_next_capture;

static void st_ring_clear(void) {
    int i;
    for (i = 0; i < ST_RING; i++) { free(st_ring[i].d); st_ring[i].d = NULL; st_ring[i].n = 0; }
    st_head = st_count = 0;
    free(st_full);
    st_full = NULL;
    st_full_len = 0;
    st_rewound = 0;
}

/* Encodes a ^ b as runs: {zeros u32, literal u32, literal bytes}. Applying it to a (in place) gives b. */
static uint8_t *st_diff(const uint8_t *a, const uint8_t *b, size_t n, size_t *out_n) {
    size_t cap = 4096, len = 0, i = 0;
    uint8_t *out = (uint8_t *)malloc(cap);
    while (out && i < n) {
        size_t z = 0, l = 0, j, zeros;
        uint32_t zz, ll;
        while (i + z < n && a[i + z] == b[i + z]) z++;
        i += z;
        j = i;
        zeros = 0;
        while (j < n && zeros < 8) { /* a literal run ends at 8 equal bytes in a row */
            if (a[j] == b[j]) zeros++; else zeros = 0;
            j++;
        }
        l = j - i - zeros;
        if (i >= n) { zz = (uint32_t)z; ll = 0; l = 0; }
        else { zz = (uint32_t)z; ll = (uint32_t)l; }
        if (len + 8 + l > cap) {
            uint8_t *nb;
            while (len + 8 + l > cap) cap *= 2;
            nb = (uint8_t *)realloc(out, cap);
            if (!nb) { free(out); return NULL; }
            out = nb;
        }
        memcpy(out + len, &zz, 4);
        memcpy(out + len + 4, &ll, 4);
        len += 8;
        for (j = 0; j < l; j++) out[len + j] = (uint8_t)(a[i + j] ^ b[i + j]);
        len += l;
        i += l;
    }
    *out_n = len;
    return out;
}

static int st_apply(uint8_t *buf, size_t n, const uint8_t *d, size_t dn) {
    size_t pos = 0, p = 0, j;
    while (p + 8 <= dn) {
        uint32_t z, l;
        memcpy(&z, d + p, 4);
        memcpy(&l, d + p + 4, 4);
        p += 8;
        if (pos + z + l > n || p + l > dn) return 0;
        pos += z;
        for (j = 0; j < l; j++) buf[pos + j] ^= d[p + j];
        pos += l;
        p += l;
    }
    return 1;
}

/* A new capture (st_data, st_len): the previous newest becomes a difference. */
static void st_ring_push(const uint8_t *data, size_t n) {
    if (st_full && st_full_len == n) {
        StDelta *slot = &st_ring[st_head];
        size_t dn;
        uint8_t *d = st_diff(data, st_full, n, &dn); /* applied to the new state it gives the previous one */
        if (d && st_echo) { /* the tests without a window check every difference by undoing it */
            uint8_t *chk = (uint8_t *)malloc(n);
            if (chk) {
                memcpy(chk, data, n);
                if (!st_apply(chk, n, d, dn) || memcmp(chk, st_full, n) != 0) printf("state: the ring difference does not give back the previous state (frame %u)\n", (unsigned)frames);
                free(chk);
            }
        }
        if (d) {
            free(slot->d);
            slot->d = d;
            slot->n = dn;
            st_head = (st_head + 1) % ST_RING;
            if (st_count < ST_RING) st_count++;
        }
    } else {
        st_ring_clear();
        st_full = (uint8_t *)malloc(n);
        if (!st_full) return;
    }
    memcpy(st_full, data, n);
    st_full_len = n;
}

/* ---- Slots on disk ---- */

static int st_path(int slot, char *out, size_t n) {
    char ext[16];
    snprintf(ext, sizeof ext, ".s%d.sst", slot);
    return st_rom_path[0] && save_path(st_rom_path, ext, out, n);
}

static int st_write_file(const char *path, const uint8_t *data, size_t n) {
    char tmp[1110];
    FILE *f;
    snprintf(tmp, sizeof tmp, "%s.tmp", path);
    f = fopen(tmp, "wb");
    if (!f) return 0;
    if (fwrite(data, 1, n, f) != n) { fclose(f); remove(tmp); return 0; }
    fclose(f);
#ifdef _WIN32
    if (!MoveFileExA(tmp, path, MOVEFILE_REPLACE_EXISTING)) { remove(tmp); return 0; }
#else
    if (rename(tmp, path) != 0) { remove(tmp); return 0; }
#endif
    return 1;
}

static void play_state_init(const char *rom_path) {
    snprintf(st_rom_path, sizeof st_rom_path, "%s", rom_path);
}

/* ---- Requests ---- */

static void st_request(int kind) {
    if (st_pending == 3 && kind != 3) { /* a request of the player goes before a capture for the rewind */
        state_req = 0;
        st_pending = 0;
    }
    if (st_pending) return;
    state_result = 0;
    if (kind == 2) { /* load the file of the slot */
        char path[1110];
        FILE *f = NULL;
        long n = 0;
        if (st_path(st_slot, path, sizeof path) && !(f = fopen(path, "rb")) && saves_dir[0]) { /* a state from before `saves` */
            char ext[16];
            snprintf(ext, sizeof ext, ".s%d.sst", st_slot);
            if (sibling_path(st_rom_path, ext, path, sizeof path)) f = fopen(path, "rb");
        }
        if (!f) {
            char t[64];
            snprintf(t, sizeof t, tr("Slot %d is empty"), st_slot);
            st_say(t);
            return;
        }
        fseek(f, 0, SEEK_END);
        n = ftell(f);
        fseek(f, 0, SEEK_SET);
        state_release();
        state_data = (uint8_t *)malloc(n > 0 ? (size_t)n : 1);
        if (state_data && fread(state_data, 1, (size_t)n, f) == (size_t)n) state_len = (size_t)n;
        else state_release();
        fclose(f);
        if (!state_data) { st_say(tr("Cannot read the state")); return; }
        st_pending = 2;
        state_req = 2;
    } else {
        st_pending = kind == 1 ? 1 : 3;
        state_req = 1;
    }
    st_since = frames;
}

/* Why nothing was done, translated when the menu is in Spanish. */
static void st_fail(const char *what) {
    if (!state_seen) { st_say(tr("Save states are not available in this game")); return; }
    st_say(what);
    snprintf(st_toast2, sizeof st_toast2, "%s", tr(state_msg));
    if (st_echo) printf("state: %s: %s (frame %u)\n", what, st_toast2, (unsigned)frames);
}

/* Once per frame, from play_frame. `want_rewind`: the rewind key is held. */
static void play_state_tick(int want_rewind, int idle) {
    char t[96];
    if (st_pending) {
        if (!state_req) { /* the runtime has dealt with it */
            int kind = st_pending;
            st_pending = 0;
            if (state_result != 1) { st_fail(tr(kind == 2 || kind == 4 ? "Cannot load" : "Cannot save")); return; }
            if (kind == 1) {
                char path[1110];
                if (st_path(st_slot, path, sizeof path) && st_write_file(path, state_data, state_len)) {
                    snprintf(t, sizeof t, tr("State saved (slot %d)"), st_slot);
                    st_say(t);
                } else st_say(tr("Cannot write the state"));
            } else if (kind == 2) {
                snprintf(t, sizeof t, tr("State loaded (slot %d)"), st_slot);
                st_say(t);
                st_ring_clear();
            } else if (kind == 3) {
                st_ring_push(state_data, state_len);
            } /* kind 4: the rewind went on */
        } else if (frames - st_since > ST_GIVE_UP_FRAMES) {
            int kind = st_pending;
            state_req = 0;
            st_pending = 0;
            if (kind <= 2) st_fail(tr(kind == 1 ? "Cannot save" : "Cannot load"));
        }
        return;
    }
    if (idle) return;
    if (want_rewind && st_full) {
        /* one captured state back per frame: the first one is the newest, then the differences are undone */
        if (st_rewound) {
            if (st_count > 0) { /* at the start of the history the oldest state is loaded again */
                st_head = (st_head + ST_RING - 1) % ST_RING;
                st_count--;
                if (!st_apply(st_full, st_full_len, st_ring[st_head].d, st_ring[st_head].n)) { st_ring_clear(); return; }
            }
        }
        st_rewound = 1;
        state_release();
        state_data = (uint8_t *)malloc(st_full_len);
        if (!state_data) return;
        memcpy(state_data, st_full, st_full_len);
        state_len = st_full_len;
        state_result = 0;
        if (st_echo) printf("state: rewind step, %d older states (frame %u)\n", st_count, (unsigned)frames);
        st_pending = 4;
        state_req = 2;
        st_since = frames;
        return;
    }
    st_rewound = 0;
    if (frames >= st_next_capture) {
        st_next_capture = frames + ST_EVERY;
        st_request(3);
    }
}

static void st_change_slot(int d) {
    char t[32];
    st_slot = (st_slot - 1 + d + ST_SLOTS) % ST_SLOTS + 1;
    snprintf(t, sizeof t, tr("Slot %d"), st_slot);
    st_say(t);
}

/* ---- Scripts for the tests without a window: --state-script save@120,load@300,slot+@50,rewind@200-230 ---- */

#define ST_SCRIPT_MAX 32
static struct { char kind; int slot_d; uint32_t lo, hi; } st_script_ev[ST_SCRIPT_MAX];
static int st_script_n;

static int parse_state_keys(const char *arg) {
    char buf[512], *tok;
    snprintf(buf, sizeof buf, "%s", arg);
    for (tok = strtok(buf, ","); tok; tok = strtok(NULL, ",")) {
        char *at = strchr(tok, '@'), *end;
        unsigned long lo, hi;
        if (!at || st_script_n == ST_SCRIPT_MAX) return 0;
        *at++ = 0;
        lo = strtoul(at, &end, 10);
        hi = *end == '-' ? strtoul(end + 1, &end, 10) : lo;
        if (*end || hi < lo) return 0;
        st_script_ev[st_script_n].lo = (uint32_t)lo;
        st_script_ev[st_script_n].hi = (uint32_t)hi;
        st_script_ev[st_script_n].slot_d = 0;
        if (!strcmp(tok, "save")) st_script_ev[st_script_n].kind = 's';
        else if (!strcmp(tok, "load")) st_script_ev[st_script_n].kind = 'l';
        else if (!strcmp(tok, "rewind")) st_script_ev[st_script_n].kind = 'r';
        else if (!strcmp(tok, "slot+")) { st_script_ev[st_script_n].kind = 'p'; st_script_ev[st_script_n].slot_d = 1; }
        else if (!strcmp(tok, "slot-")) { st_script_ev[st_script_n].kind = 'p'; st_script_ev[st_script_n].slot_d = -1; }
        else return 0;
        st_script_n++;
    }
    return 1;
}

/* Runs the events of this frame; returns whether the scripted rewind key is held. */
static int play_state_script(void) {
    int i, rew = 0;
    for (i = 0; i < st_script_n; i++) {
        if (frames < st_script_ev[i].lo || frames > st_script_ev[i].hi) continue;
        switch (st_script_ev[i].kind) {
        case 's': if (frames == st_script_ev[i].lo) st_request(1); break;
        case 'l': if (frames == st_script_ev[i].lo) st_request(2); break;
        case 'p': if (frames == st_script_ev[i].lo) st_change_slot(st_script_ev[i].slot_d); break;
        case 'r': rew = 1; break;
        }
    }
    return rew;
}

#endif
