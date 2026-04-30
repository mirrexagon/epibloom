--- Logging to Epibloom's logging infrastructure.
local record log
    enum LogLevel
        "error"
        "warn"
        "info"
        "debug"
        "trace"
    end

    log: function(level: LogLevel, message: string)

    error: function(message: string, ...: any)
    warn: function(message: string, ...: any)
    info: function(message: string, ...: any)
    debug: function(message: string, ...: any)
    trace: function(message: string, ...: any)
end

return log
