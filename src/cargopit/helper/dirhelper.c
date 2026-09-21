#include "dirhelper.h"

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <dirent.h>
#include <errno.h>

#include <pwd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <time.h>

#include <string.h>

#define DIRECTORY_MODE 0700
#define DIRECTORY_SEPARATOR_CHAR '/'
#define DIRECTORY_FORMAT "%s/%s"
#define DIRECTORY_FORMAT_WITH_SEPARATOR "%s%s"
#define USER_DIRECTORY_FORMAT "%s/%s/%s/"

static char* join_path(const char* directory, const char* entry)
{
    if (directory == NULL || entry == NULL)
    {
        return NULL;
    }

    if (directory[0] == '\0')
    {
        return strdup(entry);
    }

    size_t directory_length = strlen(directory);
    char* fullpath = NULL;
    int result;
    if (directory[directory_length - 1] == DIRECTORY_SEPARATOR_CHAR)
    {
        result = asprintf(&fullpath, DIRECTORY_FORMAT_WITH_SEPARATOR, directory, entry);
    }
    else
    {
        result = asprintf(&fullpath, DIRECTORY_FORMAT, directory, entry);
    }

    if (result < 0)
    {
        return NULL;
    }

    return fullpath;
}

void create_dir(char* dir)
{
    if (dir == NULL)
    {
        return;
    }

    struct stat st = {0};
    if (stat(dir, &st) == -1 && mkdir(dir, DIRECTORY_MODE) == -1 && errno != EEXIST)
    {
        return;
    }
}

void create_xdg_dir(const char* dir)
{
    if (dir == NULL)
    {
        return;
    }

    struct stat st = {0};
    if (stat(dir, &st) == -1 && mkdir(dir, DIRECTORY_MODE) == -1 && errno != EEXIST)
    {
        return;
    }
}

char* create_user_dir(char* home_dir_str, const char* dirtype, const char* programname)
{
    if (home_dir_str == NULL || dirtype == NULL || programname == NULL)
    {
        return NULL;
    }

    char* config_dir_str = NULL;
    if (asprintf(&config_dir_str, USER_DIRECTORY_FORMAT,
                 home_dir_str, dirtype, programname) < 0)
    {
        return NULL;
    }

    create_dir(config_dir_str);
    return config_dir_str;
}

char* gethome(void)
{
    char* homedir = getenv("HOME");
    if (homedir != NULL && homedir[0] != '\0')
    {
        return homedir;
    }

    uid_t uid = getuid();
    struct passwd* pw = getpwuid(uid);

    if (pw == NULL)
    {
        return NULL;
    }

    return pw->pw_dir;
}

void delete_dir(char* path)
{
    if (path == NULL)
    {
        return;
    }

    DIR* dr = opendir(path);
    if (dr == NULL)
    {
        return;
    }

    struct dirent* de;
    while ((de = readdir(dr)) != NULL)
    {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0)
        {
            continue;
        }

        char* fullpath = join_path(path, de->d_name);
        if (fullpath == NULL)
        {
            continue;
        }

        unlink(fullpath);
        free(fullpath);
    }

    closedir(dr);
    rmdir(path);
}

static bool delete_oldest_dir(char* path)
{
    if (path == NULL)
    {
        return false;
    }

    DIR* dr = opendir(path);
    if (dr == NULL)
    {
        return false;
    }

    struct dirent* de;
    char* deletepath = NULL;
    bool have_oldest = false;
    time_t oldest_time = 0;
    while ((de = readdir(dr)) != NULL)
    {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0)
        {
            continue;
        }

        char* fullpath = join_path(path, de->d_name);
        if (fullpath == NULL)
        {
            continue;
        }

        struct stat stbuf;
        if (stat(fullpath, &stbuf) != 0)
        {
            free(fullpath);
            continue;
        }

        time_t current_time = stbuf.st_mtime;
        if (S_ISDIR(stbuf.st_mode))
        {
            if (!have_oldest || current_time < oldest_time)
            {
                char* candidate = strdup(fullpath);
                if (candidate != NULL)
                {
                    free(deletepath);
                    deletepath = candidate;
                    oldest_time = current_time;
                    have_oldest = true;
                }
            }
        }

        free(fullpath);
    }

    closedir(dr);

    if (deletepath == NULL)
    {
        return false;
    }

    delete_dir(deletepath);
    free(deletepath);
    return true;
}

void restrict_folders_to_cache(char* path, int cachesize)
{
    if (path == NULL || cachesize < 1)
    {
        return;
    }

    int numfolders = 0;
    DIR* dr = opendir(path);
    if (dr == NULL)
    {
        return;
    }

    struct dirent* de;
    while ((de = readdir(dr)) != NULL)
    {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0)
        {
            continue;
        }

        char* fullpath = join_path(path, de->d_name);
        if (fullpath == NULL)
        {
            continue;
        }

        struct stat stbuf;
        if (stat(fullpath, &stbuf) == 0 && S_ISDIR(stbuf.st_mode))
        {
            numfolders++;
        }
        free(fullpath);
    }

    closedir(dr);

    while (numfolders > cachesize)
    {
        if (!delete_oldest_dir(path))
        {
            break;
        }
        numfolders--;
    }
}

bool does_directory_exist(char* path)
{
    DIR* dir = opendir(path);
    if (dir)
    {
        // Directory exists
        closedir(dir);
        return true;
    }
    else
    {
        // Directory does not exist or cannot be opened
        return false;
    }
}

bool does_file_exist(const char* file)
{
    if (file == NULL)
    {
        return false;
    }
#if defined(OS_WIN)
#if defined(WIN_API)
    // if you want the WinAPI, versus CRT
    if (strnlen(file, MAX_PATH+1) > MAX_PATH)
    {
        // ... throw error here or ...
        return false;
    }
    DWORD res = GetFileAttributesA(file);
    return (res != INVALID_FILE_ATTRIBUTES &&
            !(res& FILE_ATTRIBUTE_DIRECTORY));
#else
    // Use Win CRT
    struct stat fi;
    if (_stat(file, &fi) == 0)
    {
#if defined(S_ISSOCK)
        // sockets come back as a 'file' on some systems
        // so make sure it's not a socket or directory
        // (in other words, make sure it's an actual file)
        return !(S_ISDIR(fi.st_mode)) &&
               !(S_ISSOCK(fi.st_mode));
#else
        return !(S_ISDIR(fi.st_mode));
#endif
    }
    return false;
#endif
#else
    struct stat fi;
    if (stat(file, &fi) == 0)
    {
#if defined(S_ISSOCK)
        return !(S_ISDIR(fi.st_mode)) &&
               !(S_ISSOCK(fi.st_mode));
#else
        return !(S_ISDIR(fi.st_mode));
#endif
    }
    return false;
#endif
}

char* expand_tilde(char* path)
{
    if (path == NULL || path[0] != '~')
    {
        return path;
    }

    const char* home_dir = getenv("HOME");
    if (!home_dir) {
        return path;
    }

    char* expanded_path = NULL;
    if (asprintf(&expanded_path, "%s%s", home_dir, path + 1) < 0)
    {
        return path;
    }

    free(path);
    return expanded_path;
}
