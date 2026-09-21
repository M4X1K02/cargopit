#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <sys/time.h>
#include <limits.h>
#include <stdint.h>

#include "arduinoledlua.h"
#include "arduino.h"

#include "../../slog/slog.h"

#define LUA_TOTAL_LEDS_NAME "TotalLeds"
#define LUA_LED_BUFFER_NAME "buff"
#define RGB_CHANNEL_RED 0
#define RGB_CHANNEL_GREEN 1
#define RGB_CHANNEL_BLUE 2
#define LED_COLOR_FULL UINT8_MAX
#define LED_COLOR_ORANGE_GREEN 165

static int get_total_leds(lua_State* L)
{
    if (L == NULL)
    {
        return 0;
    }

    lua_getglobal(L, LUA_TOTAL_LEDS_NAME);
    if (!lua_isnumber(L, -1))
    {
        lua_pop(L, 1);
        return 0;
    }

    lua_Integer total_leds = lua_tointeger(L, -1);
    lua_pop(L, 1);
    if (total_leds <= 0 || total_leds > INT_MAX)
    {
        return 0;
    }

    return (int)total_leds;
}

static uint8_t* get_led_buffer(lua_State* L)
{
    if (L == NULL)
    {
        return NULL;
    }

    lua_pushstring(L, LUA_LED_BUFFER_NAME);
    lua_gettable(L, LUA_REGISTRYINDEX);
    uint8_t* bytes = (uint8_t*)lua_touserdata(L, -1);
    lua_pop(L, 1);
    return bytes;
}

long long ledTimeInMilliseconds(void)
{
    struct timeval tv;

    gettimeofday(&tv,NULL);
    return (((long long)tv.tv_sec)*1000)+(tv.tv_usec/1000);
}


int simdata_to_lua(lua_State *L, SimData* simdata) {

    // make the time up now if we are running in test mode
    if(simdata->mtick == 0)
    {
        simdata->mtick = ledTimeInMilliseconds();
    }

    lua_newtable(L);

    lua_pushstring(L, simdata->gearc);
    lua_setfield(L, -2, "gearc");

    lua_pushinteger(L, simdata->playerflag);
    lua_setfield(L, -2, "playerflag");

    lua_pushinteger(L, simdata->rpms);
    lua_setfield(L, -2, "rpm");
    
    lua_pushinteger(L, simdata->gear);
    lua_setfield(L, -2, "gear");
    
    lua_pushinteger(L, simdata->velocity);
    lua_setfield(L, -2, "velocity");

    lua_pushinteger(L, simdata->mtick);
    lua_setfield(L, -2, "mtick");

    lua_pushinteger(L, simdata->maxrpm);
    lua_setfield(L, -2, "maxrpm");

    lua_pushinteger(L, PROXCARS);
    lua_setfield(L, -2, "proxcars");

    lua_newtable(L);
    for(int i = 0; i < PROXCARS; i++)
    {
        lua_newtable(L);
        lua_pushinteger(L, simdata->pd[i].radius);
        lua_setfield(L, -2, "radius");
        lua_pushinteger(L, simdata->pd[i].theta);
        lua_setfield(L, -2, "theta");
        lua_rawseti(L, -2, i+1);
    }
    lua_setfield(L, -2, "pd");

    lua_pushnumber(L, simdata->gas);
    lua_setfield(L, -2, "gas");

    lua_pushnumber(L, simdata->fuel);
    lua_setfield(L, -2, "fuel");

    lua_pushnumber(L, simdata->turboboost);
    lua_setfield(L, -2, "turboboost");

    lua_newtable(L);
    for(int i = 0; i < 4; i++)
    {
        lua_pushnumber(L, simdata->tyreRPS[i]);
        lua_rawseti(L, -2, i+1);
    }
    lua_setfield(L, -2, "tyreRPS");

    lua_newtable(L);
    for(int i = 0; i < 4; i++)
    {
        lua_pushnumber(L, simdata->tyrediameter[i]);
        lua_rawseti(L, -2, i+1);
    }
    lua_setfield(L, -2, "tyrediameter");

    lua_newtable(L);
    for(int i = 0; i < 4; i++)
    {
        lua_pushnumber(L, simdata->tyretemp[i]);
        lua_rawseti(L, -2, i+1);
    }
    lua_setfield(L, -2, "tyretemp");

    return 0; // Return the table to Lua
}

uint8_t get_color_rgb_value(int color, int rgb)
{
    switch (color)
    {
        case LUALEDCOLOR_RED:
            return rgb == RGB_CHANNEL_RED ? LED_COLOR_FULL : 0;
        case LUALEDCOLOR_GREEN:
            return rgb == RGB_CHANNEL_GREEN ? LED_COLOR_FULL : 0;
        case LUALEDCOLOR_BLUE:
            return rgb == RGB_CHANNEL_BLUE ? LED_COLOR_FULL : 0;
        case LUALEDCOLOR_YELLOW:
            if (rgb == RGB_CHANNEL_RED || rgb == RGB_CHANNEL_GREEN)
            {
                return LED_COLOR_FULL;
            }
            return 0;
        case LUALEDCOLOR_ORANGE:
            if (rgb == RGB_CHANNEL_RED)
            {
                return LED_COLOR_FULL;
            }
            return rgb == RGB_CHANNEL_GREEN ? LED_COLOR_ORANGE_GREEN : 0;
        default:
            return 0;
    }
}

int set_led_range_to_color(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function set_led_range_to_color");

    int range_start = lua_tonumber(L, 1);
    int range_end = lua_tonumber(L, 2);
    int color = lua_tonumber(L, 3);

    slogd("lua range start is %i", range_start);
    slogd("lua range end is %i", range_end);
    slogd("lua color is %i", color);

    range_start = range_start - 1;

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    if (range_start < 0 || range_end <= range_start || range_end > numlights)
    {
        if (range_end > numlights)
        {
            range_end = numlights;
        }
        if (range_start < 0 || range_end <= range_start)
        {
            slogt("Invalid range, doing nothing");
            return 1;
        }
    }

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }

    slogt("first byte of buff is x%02x", bytes[0]);

    uint8_t color0 = get_color_rgb_value(color, RGB_CHANNEL_RED);
    uint8_t color1 = get_color_rgb_value(color, RGB_CHANNEL_GREEN);
    uint8_t color2 = get_color_rgb_value(color, RGB_CHANNEL_BLUE);

    for( int i = range_start; i < range_end; i++)
    {
        bytes[(i * 3) + 0] = color0;
        bytes[(i * 3) + 1] = color1;
        bytes[(i * 3) + 2] = color2;
    }

    return 0;
}

int set_led_range_to_rgb_color(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function set_led_range_to_rgb_color");

    int range_start = lua_tonumber(L, 1);
    int range_end = lua_tonumber(L, 2);
    int color = lua_tonumber(L, 3);

    uint8_t color0 = (color >> 16) & 0xff;
    uint8_t color1 = (color >> 8) & 0xff;
    uint8_t color2 = (color >> 0) & 0xff;
    //int color0 = lua_tonumber(L, 3);
    //int color1 = lua_tonumber(L, 4);
    //int color2 = lua_tonumber(L, 5);

    slogd("lua range start is %i", range_start);
    slogd("lua range end is %i", range_end);
    slogd("lua color0 is %i", color0);
    slogd("lua color1 is %i", color1);
    slogd("lua color2 is %i", color2);

    range_start = range_start - 1;

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    if (range_start < 0 || range_end <= range_start || range_end > numlights)
    {
        if (range_end > numlights)
        {
            range_end = numlights;
        }
        if (range_start < 0 || range_end <= range_start)
        {
            return 1;
        }
    }

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }

    slogt("first byte of buff is x%02x", bytes[0]);

    for( int i = range_start; i < range_end; i++)
    {
        bytes[(i * 3) + 0] = color0;
        bytes[(i * 3) + 1] = color1;
        bytes[(i * 3) + 2] = color2;
    }

    return 0;
}

int set_led_to_color(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function set_led_to_rgb_color");

    int led = lua_tonumber(L, 1);
    int color = lua_tonumber(L, 2);

    slogd("lua led is %i", led);
    slogd("lua color is %i", color);

    led = led - 1;

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    if (led < 0 || led >= numlights)
    {
        return 1;
    }

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }
    slogt("first byte of buff is x%02x", bytes[0]);

    uint8_t color0 = get_color_rgb_value(color, RGB_CHANNEL_RED);
    uint8_t color1 = get_color_rgb_value(color, RGB_CHANNEL_GREEN);
    uint8_t color2 = get_color_rgb_value(color, RGB_CHANNEL_BLUE);

    bytes[(led * 3) + 0] = color0;
    bytes[(led * 3) + 1] = color1;
    bytes[(led * 3) + 2] = color2;

    return 0;

}


int set_led_to_rgb_color(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function set_led_to_rgb_color");

    int led = lua_tonumber(L, 1);
    int color = lua_tonumber(L, 2);

    uint8_t color0 = (color >> 16) & 0xff;
    uint8_t color1 = (color >> 8) & 0xff;
    uint8_t color2 = (color >> 0) & 0xff;

    slogd("lua led is %i", led);
    slogd("lua color0 is %i", color0);
    slogd("lua color1 is %i", color1);
    slogd("lua color2 is %i", color2);

    led = led - 1;

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    if (led < 0 || led >= numlights)
    {
        return 1;
    }

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }
    slogt("first byte of buff is x%02x", bytes[0]);

    bytes[(led * 3) + 0] = color0;
    bytes[(led * 3) + 1] = color1;
    bytes[(led * 3) + 2] = color2;

    return 0;

}


int led_clear_all(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function led_clear_all");

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }
    slogt("first byte of buff is x%02x", bytes[0]);

    for( int i = 0; i < numlights; i++)
    {
        bytes[(i * 3) + 0] = 0x00;
        bytes[(i * 3) + 1] = 0x00;
        bytes[(i * 3) + 2] = 0x00;
    }

    return 0;
}

int led_clear_range(lua_State *L)
{
    if (L == NULL)
    {
        return 1;
    }

    slogt("lua called c function led_clear_range");

    int range_start = lua_tonumber(L, 1);
    int range_end = lua_tonumber(L, 2);

    slogd("lua range start is %i", range_start);
    slogd("lua range end is %i", range_end);

    range_start = range_start - 1;

    int numlights = get_total_leds(L);
    slogd("num leds is %i", numlights);

    if (range_start < 0 || range_end <= range_start || range_end > numlights)
    {
        if (range_end > numlights)
        {
            range_end = numlights;
        }
        if (range_start < 0 || range_end <= range_start)
        {
            return 1;
        }
    }

    uint8_t* bytes = get_led_buffer(L);
    if (bytes == NULL)
    {
        slogt("LED buffer is unavailable");
        return 1;
    }

    slogt("first byte of buff is x%02x", bytes[0]);

    for (int i = range_start; i < range_end; i++)
    {
        bytes[(i * 3) + 0] = 0x00;
        bytes[(i * 3) + 1] = 0x00;
        bytes[(i * 3) + 2] = 0x00;
    }

    return 0;
}
