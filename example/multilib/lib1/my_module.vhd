library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

library lib2;

entity my_module is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        a     : in std_logic;
        b     : out std_logic
    );
end entity my_module;

architecture rtl of my_module is

    component a_submodule is
    port (
        clk   : in std_logic;
        reset : in std_logic;
        
        c     : in std_logic;
        d     : out std_logic
    );
    end component a_submodule;

    signal d : std_logic;

begin

    submodule_inst: a_submodule
     port map(
        clk => clk,
        reset => reset,
        c => a,
        d => d
    );
    
    my_module_inst: entity lib2.my_module
     port map(
        clk => clk,
        reset => reset,
        d => d,
        b => b
    );
end architecture rtl;