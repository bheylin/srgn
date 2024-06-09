provider "aws" {
  region = var.aws_region
}


variable "aws_region" {
  type        = string
  default     = "us-east-1"
  description = <<EOT
AWS region for the provider and resources.

Restricted to specific regions, and to ${var.resource_prefix} resources.
EOT

  validation {
    condition     = contains(["us-east-1", "us-west-1", "eu-central-1"], var.aws_region)
    error_message = "The specified region is not supported."
  }
}

variable "resource_prefix" {
  default = "prod"
}

variable "resource_name" {
  default = "${var.resource_prefix}-resource"
}

resource "aws_instance" "example" {
  ami               = "ami-12345678"
  instance_type     = var.aws_region == "us-west-1" ? "t2.micro" : "t2.small"
  availability_zone = "${var.aws_region}a"
}

module "network" {
  source = "./modules/network"
  region = var.aws_region
}

variable "cidrs" {
  type    = list(string)
  default = ["10.0.0.0/24", "10.0.1.0/24"]
}

resource "aws_vpc" "example" {
  cidr_block = var.cidrs[0]
}

output "aws_region" {
  value     = var.aws_region
  sensitive = true
}