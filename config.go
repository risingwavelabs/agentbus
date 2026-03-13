package main

import (
	"fmt"
	"os"

	"github.com/kelseyhightower/envconfig"
	"gopkg.in/yaml.v3"
)

// Config holds all configuration
type Config struct {
	Server ServerConfig `yaml:"server" envconfig:"STREAM0_SERVER"`
	DB     DBConfig     `yaml:"database" envconfig:"STREAM0_DB"`
	Log    LogConfig    `yaml:"log" envconfig:"STREAM0_LOG"`
}

// ServerConfig holds server configuration
type ServerConfig struct {
	Host string `yaml:"host" envconfig:"HOST" default:"127.0.0.1"`
	Port int    `yaml:"port" envconfig:"PORT" default:"8080"`
}

// DBConfig holds database configuration
type DBConfig struct {
	Path string `yaml:"path" envconfig:"PATH" default:"./stream0.db"`
}

// LogConfig holds logging configuration
type LogConfig struct {
	Level  string `yaml:"level" envconfig:"LEVEL" default:"info"`
	Format string `yaml:"format" envconfig:"FORMAT" default:"json"`
}

// LoadConfig loads configuration from file and environment variables
func LoadConfig(path string) (*Config, error) {
	var cfg Config

	// Set defaults
	cfg.Server.Host = "127.0.0.1"
	cfg.Server.Port = 8080
	cfg.DB.Path = "./stream0.db"
	cfg.Log.Level = "info"
	cfg.Log.Format = "json"

	// Load from file if provided
	if path != "" {
		data, err := os.ReadFile(path)
		if err == nil {
			if err := yaml.Unmarshal(data, &cfg); err != nil {
				return nil, fmt.Errorf("failed to parse config file: %w", err)
			}
		}
	}

	// Override with environment variables
	if err := envconfig.Process("stream0", &cfg); err != nil {
		return nil, fmt.Errorf("failed to process env vars: %w", err)
	}

	return &cfg, nil
}

// Address returns the server address
func (c *ServerConfig) Address() string {
	return fmt.Sprintf("%s:%d", c.Host, c.Port)
}
